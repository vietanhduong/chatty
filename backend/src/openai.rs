#[cfg(test)]
#[path = "openai_test.rs"]
mod tests;

use std::ops::Deref;
use std::sync::Arc;
use std::{fmt::Display, time};

use crate::{ArcBackend, Backend, TITLE_PROMPT};
use async_trait::async_trait;
use eyre::{Context, Result, bail};
use futures::TryStreamExt;
use openai_models::{
    BackendConnection, BackendPrompt, BackendResponse, BackendUsage, Event, Message,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncBufReadExt;
use tokio::sync::{RwLock, mpsc};
use tokio_util::io::StreamReader;

#[derive(Debug)]
pub struct OpenAI {
    alias: String,
    endpoint: String,
    api_key: Option<String>,
    timeout: Option<time::Duration>,

    want_models: Vec<String>,

    cache_models: RwLock<Vec<String>>,
    current_model: RwLock<Option<String>>,
}

#[async_trait]
impl Backend for OpenAI {
    fn name(&self) -> &str {
        &self.alias
    }

    async fn health_check(&self) -> Result<()> {
        if self.endpoint.is_empty() {
            bail!("Endpoint is not set");
        }

        let cached_models = self.cache_models.read().await;
        if !cached_models.is_empty() {
            return Ok(());
        }
        drop(cached_models);

        self.list_models(true).await?;
        Ok(())
    }

    async fn list_models(&self, force: bool) -> Result<Vec<String>> {
        if !force && !self.cache_models.read().await.is_empty() {
            return Ok(self.cache_models.read().await.clone());
        }

        let mut req = reqwest::Client::new().get(format!("{}/v1/models", self.endpoint));

        if let Some(timeout) = self.timeout {
            req = req.timeout(timeout);
        }

        if let Some(token) = &self.api_key {
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

        let all = self.want_models.is_empty();

        let mut models = res
            .data
            .into_iter()
            .filter(|m| all || self.want_models.contains(&m.id))
            .map(|m| m.id)
            .collect::<Vec<_>>();

        models.sort();

        let mut cached = self.cache_models.write().await;
        *cached = models.clone();
        Ok(models)
    }

    async fn current_model(&self) -> Option<String> {
        let default_model = self.current_model.read().await;
        return default_model.clone();
    }

    async fn set_current_model(&self, model: &str) -> Result<()> {
        // We will check the model against the list of available models
        // If the model is not available, we will return an error
        let models = self.list_models(false).await?;
        let model = if model.is_empty() {
            models
                .last()
                .ok_or_else(|| eyre::eyre!("no models available"))?
        } else {
            models
                .iter()
                .find(|m| m == &model)
                .ok_or_else(|| eyre::eyre!("model {} not available", model))?
        };
        let mut default_model = self.current_model.write().await;
        *default_model = Some(model.clone());
        Ok(())
    }

    async fn get_completion<'a>(
        &self,
        prompt: BackendPrompt,
        event_tx: &'a mpsc::UnboundedSender<Event>,
    ) -> Result<()> {
        if self.current_model().await.is_none() && prompt.model().is_none() {
            bail!("no model is set");
        }

        let mut messages: Vec<MessageRequest> = vec![];
        if !prompt.context().is_empty() {
            // FIXME: This approach might not be optimized for large contexts
            messages = prompt
                .context()
                .into_iter()
                .map(MessageRequest::from)
                .collect::<Vec<_>>();
        }

        let init_conversation = prompt.context().is_empty();
        let content = if init_conversation {
            format!("{}\n{}", prompt.text(), TITLE_PROMPT)
        } else {
            prompt.text().to_string()
        };

        messages.push(MessageRequest {
            role: "user".to_string(),
            content,
        });

        let model = match prompt.model() {
            Some(model) => model.to_string(),
            None => self.current_model().await.unwrap(),
        };

        let completion_req = CompletionRequest {
            model: model.clone(),
            messages: messages.clone(),
            stream: true,
        };

        let mut req = reqwest::Client::new()
            .post(format!("{}/v1/chat/completions", self.endpoint))
            .header("Content-Type", "application/json");

        if let Some(timeout) = self.timeout {
            req = req.timeout(timeout);
        }

        if let Some(token) = &self.api_key {
            req = req.bearer_auth(token);
        }

        log::trace!("Sending completion request: {:?}", completion_req);

        let res = req
            .json(&completion_req)
            .send()
            .await
            .wrap_err("sending completion request")?;

        if !res.status().is_success() {
            let http_code = res.status().as_u16();
            let resp = res.text().await.wrap_err("parsing error response")?;
            let err = serde_json::from_str::<ErrorResponse>(&resp)
                .wrap_err(format!("parsing error response: {}", resp))?;
            let mut err = err.error;
            err.http_code = http_code;
            return Err(err.into());
        }

        let stream = res.bytes_stream().map_err(|e| {
            let err_msg = e.to_string();
            return std::io::Error::new(std::io::ErrorKind::Interrupted, err_msg);
        });

        let mut line_readers = StreamReader::new(stream).lines();

        let mut message_id = String::new();
        let mut usage: Option<BackendUsage> = None;

        while let Ok(line) = line_readers.next_line().await {
            if line.is_none() {
                break;
            }

            let mut line = line.unwrap().trim().to_string();
            if line.starts_with("data: ") {
                line = line[6..].to_string();
            }

            if line.ends_with(": keep-alive") || line.is_empty() {
                continue;
            }

            if line == "[DONE]" {
                break;
            }

            log::trace!("streaming response: {}", line);

            let data = serde_json::from_str::<CompletionResponse>(&line)
                .wrap_err(format!("parsing completion response line: {}", line))?;

            let c = match data.choices.get(0) {
                Some(c) => c,
                None => continue,
            };

            if message_id.is_empty() {
                message_id = data.id;
            }

            if c.finish_reason.is_some() {
                if let Some(usage_data) = data.usage {
                    usage = Some(BackendUsage {
                        prompt_tokens: usage_data.prompt_tokens,
                        completion_tokens: usage_data.completion_tokens,
                        total_tokens: usage_data.total_tokens,
                    });
                }
                break;
            }

            let text = match c.delta.content {
                Some(ref text) => text.deref().to_string(),
                None => continue,
            };

            let msg = BackendResponse {
                id: message_id.clone(),
                model: model.clone(),
                text,
                done: false,
                init_conversation,
                usage: None,
            };
            event_tx.send(Event::BackendPromptResponse(msg))?;
        }

        let msg = BackendResponse {
            id: message_id,
            model: model.clone(),
            text: String::new(),
            done: true,
            init_conversation,
            usage,
        };
        event_tx.send(Event::BackendPromptResponse(msg))?;
        Ok(())
    }
}

impl From<OpenAI> for ArcBackend {
    fn from(value: OpenAI) -> Self {
        Arc::new(value)
    }
}

impl From<&BackendConnection> for OpenAI {
    fn from(value: &BackendConnection) -> Self {
        let mut openai = OpenAI::default().with_endpoint(value.endpoint());

        if let Some(api_key) = value.api_key() {
            openai.api_key = Some(api_key.to_string());
        }

        if let Some(timeout) = value.timeout() {
            openai.timeout = Some(timeout);
        }

        if let Some(alias) = value.alias() {
            openai.alias = alias.to_string();
        }

        openai.want_models = value.models().to_vec();
        openai
    }
}

impl OpenAI {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_want_models(mut self, models: Vec<String>) -> Self {
        self.want_models = models;
        self
    }

    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = endpoint.to_string();
        self
    }

    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.api_key = Some(api_key.to_string());
        self
    }

    pub fn with_timeout(mut self, timeout: time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    pub fn timeout(&self) -> Option<time::Duration> {
        self.timeout
    }

    pub fn want_models(&self) -> &[String] {
        &self.want_models
    }
}

impl Default for OpenAI {
    fn default() -> Self {
        Self {
            alias: "OpenAI".to_string(),
            endpoint: "https://api.openai.com".to_string(),
            api_key: None,
            timeout: None,
            current_model: RwLock::new(None),
            cache_models: tokio::sync::RwLock::new(Vec::new()),
            want_models: vec![],
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
    usage: Option<CompletionUsageResponse>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CompletionUsageResponse {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct ErrorResponse {
    error: OpenAIError,
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

impl From<&Message> for MessageRequest {
    fn from(value: &Message) -> Self {
        Self {
            role: if value.is_system() {
                "assistant".to_string()
            } else {
                "user".to_string()
            },
            content: value.text().to_string(),
        }
    }
}
