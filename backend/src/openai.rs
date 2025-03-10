use std::ops::Deref;
use std::{fmt::Display, time};

use crate::Backend;
use async_trait::async_trait;
use eyre::{Context, Result, bail};
use futures::stream::StreamExt;
use log::{debug, error, trace};
use openai_models::message::Issuer;
use openai_models::{BackendPrompt, BackendResponse, Event, Message};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct OpenAI {
    endpoint: String,
    token: Option<String>,
    timeout: Option<time::Duration>,

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
        let mut req = reqwest::Client::new().get(format!("{}/v1/models", self.endpoint));

        if let Some(timeout) = self.timeout {
            req = req.timeout(timeout);
        }

        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
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
            // FIXME: This approach might not be optimized for large contexts
            messages = prompt.context().iter().map(MessageRequest::from).collect();
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
            .header("Content-Type", "application/json");

        if let Some(timeout) = self.timeout {
            req = req.timeout(timeout);
        }

        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }

        debug!("sending completion request: {:?}", completion_req);

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

        let mut stream = res.bytes_stream();

        let mut last_message = String::new();
        let mut message_id = String::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            let s = match std::str::from_utf8(&item) {
                Ok(s) => s,
                Err(_) => {
                    error!("Error parsing stream response");
                    continue;
                }
            };

            for p in s.split("\n\n") {
                match p.strip_prefix("data: ") {
                    Some(p) => {
                        if p == "[DONE]" {
                            break;
                        }

                        trace!("streaming response: body: {}", p);
                        let data = serde_json::from_str::<CompletionResponse>(p)
                            .wrap_err("parsing completion response")?;

                        let c = match data.choices.get(0) {
                            Some(c) => c,
                            None => continue,
                        };

                        if message_id.is_empty() {
                            message_id = data.id;
                        }

                        if c.finish_reason.is_some() {
                            break;
                        }

                        let text = match c.delta.content {
                            Some(ref text) => text.deref().to_string(),
                            None => continue,
                        };

                        last_message += &text;
                        let msg = BackendResponse {
                            id: message_id.clone(),
                            model: self.current_model().to_string(),
                            text,
                            context: vec![],
                            done: false,
                        };

                        event_tx.send(Event::BackendPromptResponse(msg))?;
                    }
                    None => {}
                }
            }
        }

        messages.push(MessageRequest {
            role: "assistant".to_string(),
            content: last_message.clone(),
        });

        let mut context = prompt.context().to_vec();
        context.push(Message::new_system(&self.current_model, last_message).with_id(&message_id));

        let msg = BackendResponse {
            id: message_id,
            model: self.current_model().to_string(),
            text: String::new(),
            context,
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
        self.timeout = Some(timeout);
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub fn timeout(&self) -> Option<time::Duration> {
        self.timeout
    }
}

impl Default for OpenAI {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com".to_string(),
            token: None,
            timeout: None,
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
    id: String,
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

impl From<Message> for MessageRequest {
    fn from(value: Message) -> Self {
        Self {
            role: match value.issuer() {
                Issuer::System(_) => "assistant".to_string(),
                Issuer::User(_) => "user".to_string(),
            },
            content: value.text().to_string(),
        }
    }
}

impl From<&Message> for MessageRequest {
    fn from(value: &Message) -> Self {
        Self {
            role: match value.issuer() {
                Issuer::System(_) => "assistant".to_string(),
                Issuer::User(_) => "user".to_string(),
            },
            content: value.text().to_string(),
        }
    }
}
