use std::{fmt::Display, time};

use crate::Backend;
use async_trait::async_trait;
use eyre::{Context, Result, bail};
use futures::stream::TryStreamExt;
use openai_models::{BackendPrompt, BackendResponse, Event};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;
use tokio_util::io::StreamReader;

#[derive(Debug)]
pub struct OpenAI {
    endpoint: String,
    token: Option<String>,
    timeout: time::Duration,

    current_model: String,
}

#[async_trait]
impl Backend for OpenAI {
    async fn health_check(&self) -> Result<()> {
        if self.endpoint.is_empty() {
            bail!("Endpoint is not set");
        }
        self.list_models().await?;
        Ok(())
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let mut req = reqwest::Client::new()
            .get(format!("{}/v1/models", self.endpoint))
            .timeout(self.timeout);

        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let res = req.send().await.wrap_err("listing models")?;

        if !res.status().is_success() {
            let http_code = res.status().as_u16();
            let err: ErrorResponse = res.json().await.wrap_err("parsing error response")?;
            let mut err = err.error;
            err.http_code = http_code;
            return Err(err.into());
        }

        let res = res
            .json::<ModelListResponse>()
            .await
            .wrap_err("parsing model list response")?;

        let mut models = res
            .data
            .iter()
            .map(|m| m.id.to_string())
            .collect::<Vec<_>>();

        models.sort();
        Ok(models)
    }

    fn current_model(&self) -> &str {
        &self.current_model
    }

    async fn set_model(&mut self, model: &str) -> Result<()> {
        // We will check the model against the list of available models
        // If the model is not available, we will return an error
        let models = self.list_models().await?;
        if !model.is_empty() && !models.contains(&model.to_string()) {
            bail!("OpenAI error: Model {} is not available", model);
        }

        let model = if model.is_empty() {
            models
                .last()
                .ok_or_else(|| eyre::eyre!("OpenAI error: No models available"))?
        } else {
            model
        };
        self.current_model = model.to_string();
        Ok(())
    }

    async fn get_completion<'a>(
        &self,
        prompt: BackendPrompt,
        event_tx: &'a mpsc::UnboundedSender<Event>,
    ) -> Result<()> {
        if self.current_model().is_empty() {
            bail!("OpenAI error: Model is not set");
        }

        let mut messages: Vec<MessageRequest> = vec![];
        if !prompt.context().is_empty() {
            messages = serde_json::from_str(&prompt.context()).wrap_err("parsing context")?;
        }

        messages.push(MessageRequest {
            role: "user".to_string(),
            content: prompt.text().to_string(),
        });

        let completion_req = CompletionRequest {
            model: self.current_model().to_string(),
            messages: messages.clone(),
            stream: true,
        };

        let mut req = reqwest::Client::new()
            .post(format!("{}/v1/chat/completions", self.endpoint))
            .header("Content-Type", "application/json")
            .timeout(self.timeout);

        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let res = req
            .json(&completion_req)
            .send()
            .await
            .wrap_err("sending completion request")?;

        if !res.status().is_success() {
            let http_code = res.status().as_u16();
            let err: ErrorResponse = res.json().await.wrap_err("parsing error response")?;
            let mut err = err.error;
            err.http_code = http_code;
            return Err(err.into());
        }

        let stream = res.bytes_stream().map_err(|e| -> std::io::Error {
            std::io::Error::new(std::io::ErrorKind::Interrupted, e.to_string())
        });
        let mut lines_reader = StreamReader::new(stream).lines();
        let mut last_message = String::new();
        while let Ok(line) = lines_reader.next_line().await {
            if line.is_none() {
                break;
            }

            let mut cleaned_line = line.unwrap().trim().to_string();
            if cleaned_line.starts_with("data:") {
                cleaned_line = cleaned_line[5..].trim().to_string();
            }
            if cleaned_line.is_empty() {
                continue;
            }
            let ores: CompletionResponse =
                serde_json::from_str(&cleaned_line).wrap_err("parsing completion response")?;
            tracing::debug!(body = ?ores, "streaming response");

            let choice = &ores.choices[0];
            if choice.finish_reason.is_some() {
                break;
            }
            if choice.delta.content.is_none() {
                continue;
            }

            let text = choice.delta.content.clone().unwrap().to_string();
            if text.is_empty() {
                continue;
            }

            last_message += &text;
            let msg = BackendResponse {
                model: self.current_model().to_string(),
                text,
                context: None,
                done: false,
            };

            event_tx.send(Event::BackendPromptResponse(msg))?;
        }

        messages.push(MessageRequest {
            role: "assistant".to_string(),
            content: last_message.clone(),
        });

        let msg = BackendResponse {
            model: self.current_model().to_string(),
            text: String::new(),
            context: Some(serde_json::to_string(&messages).wrap_err("serializing context")?),
            done: true,
        };
        event_tx.send(Event::BackendPromptResponse(msg))?;
        Ok(())
    }
}

impl OpenAI {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = endpoint.to_string();
        self
    }

    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }

    pub fn with_timeout(mut self, timeout: time::Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn timeout(&self) -> time::Duration {
        self.timeout
    }
}

impl Default for OpenAI {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com".to_string(),
            token: None,
            timeout: time::Duration::from_secs(30),
            current_model: String::new(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Model {
    id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ModelListResponse {
    data: Vec<Model>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct ErrorResponse {
    error: OpenAIError,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MessageRequest {
    role: String,
    content: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CompletionRequest {
    model: String,
    messages: Vec<MessageRequest>,
    stream: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CompletionDeltaResponse {
    content: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CompletionChoiceResponse {
    delta: CompletionDeltaResponse,
    finish_reason: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CompletionResponse {
    choices: Vec<CompletionChoiceResponse>,
}

#[derive(Default, Error, Debug, Serialize, Deserialize)]
pub struct OpenAIError {
    #[serde(skip)]
    pub http_code: u16,
    pub message: String,
    #[serde(rename = "type")]
    pub err_type: String,
    pub param: Option<String>,
    pub code: Option<String>,
}

impl Display for OpenAIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpenAI error ({}): {}", self.http_code, self.message)
    }
}
