#[cfg(test)]
#[path = "openai_test.rs"]
mod tests;

use crate::backend::utils::context_truncation;
use crate::backend::{ArcBackend, Backend, TITLE_PROMPT};
use crate::config::user_agent;
use crate::models::ToolInputSchema;
use crate::models::{
    ArcEventTx, BackendConnection, BackendPrompt, BackendResponse, BackendUsage, Event, Message,
    Model,
};
use async_trait::async_trait;
use eyre::{Context, Result, bail};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::Arc;
use std::{fmt::Display, time};
use thiserror::Error;
use tokio::io::AsyncBufReadExt;
use tokio_util::io::StreamReader;

use super::mcp;

pub struct OpenAI {
    alias: String,
    endpoint: String,
    api_key: Option<String>,
    timeout: Option<time::Duration>,
    mcp: Option<Arc<dyn mcp::MCP>>,

    want_models: Vec<String>,

    max_output_tokens: Option<usize>,
}

#[async_trait]
impl Backend for OpenAI {
    fn name(&self) -> &str {
        &self.alias
    }

    async fn list_models(&self) -> Result<Vec<Model>> {
        let mut req = reqwest::Client::new()
            .get(format!("{}/v1/models", self.endpoint))
            .header("User-Agent", user_agent());

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
            .map(|m| Model::new(m.id).with_provider(&self.alias))
            .collect::<Vec<_>>();

        models.sort_by(|a, b| a.id().cmp(b.id()));

        Ok(models)
    }

    async fn get_completion(&self, prompt: BackendPrompt, event_tx: ArcEventTx) -> Result<()> {
        if prompt.model().is_empty() {
            bail!("no model is set");
        }

        let init_conversation = prompt.context().is_empty();
        let content = if init_conversation && !prompt.no_generate_title() {
            format!("{}\n{}", prompt.text(), TITLE_PROMPT)
        } else {
            prompt.text().to_string()
        };

        let mut messages = prompt.context().to_vec();
        messages.push(Message::new_user("user", content));

        if let Some(max_output_tokens) = self.max_output_tokens {
            context_truncation(&mut messages, max_output_tokens);
        }

        let messages = messages
            .into_iter()
            .map(|m| MessageRequest::from(&m))
            .collect::<Vec<_>>();

        let completion_req = CompletionRequest {
            model: prompt.model().to_string(),
            messages: messages.clone(),
            stream: true,
            max_completion_tokens: self.max_output_tokens,
            tools: vec![],
        };

        let mut req = reqwest::Client::new()
            .post(format!("{}/v1/chat/completions", self.endpoint))
            .header("Content-Type", "application/json")
            .header("User-Agent", user_agent());

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
            log::trace!("streaming response: {}", line);
            if !line.starts_with("data: ") {
                continue;
            }

            line = line[6..].to_string();
            if line == "[DONE]" {
                break;
            }

            let data = serde_json::from_str::<CompletionResponse>(&line)
                .wrap_err(format!("parsing completion response line: {}", line))?;

            let c = match data.choices.get(0) {
                Some(c) => c,
                None => continue,
            };

            if message_id.is_empty() {
                message_id = data.id;
            }

            let text = match c.delta.content {
                Some(ref text) => text.deref().to_string(),
                None => continue,
            };

            let msg = BackendResponse {
                id: message_id.clone(),
                model: prompt.model().to_string(),
                text,
                done: false,
                init_conversation,
                usage: None,
            };
            event_tx.send(Event::BackendPromptResponse(msg)).await?;

            if let Some(usage_data) = data.usage {
                usage = Some(BackendUsage {
                    prompt_tokens: usage_data.prompt_tokens,
                    completion_tokens: usage_data.completion_tokens,
                    total_tokens: usage_data.total_tokens,
                });
            }
        }

        let msg = BackendResponse {
            id: message_id,
            model: prompt.model().to_string(),
            text: String::new(),
            done: true,
            init_conversation,
            usage,
        };
        event_tx.send(Event::BackendPromptResponse(msg)).await?;
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

        openai.max_output_tokens = value.max_output_tokens();

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

    pub fn with_mcp(mut self, mcp: Arc<dyn mcp::MCP>) -> Self {
        self.mcp = Some(mcp);
        self
    }
}

impl Default for OpenAI {
    fn default() -> Self {
        Self {
            max_output_tokens: None,
            alias: "OpenAI".to_string(),
            endpoint: "https://api.openai.com".to_string(),
            api_key: None,
            timeout: None,
            want_models: vec![],
            mcp: None,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct ModelResponse {
    id: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct ModelListResponse {
    data: Vec<ModelResponse>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct MessageRequest {
    role: String,
    content: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct CompletionRequest {
    model: String,
    messages: Vec<MessageRequest>,
    stream: bool,
    max_completion_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolRequest>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct ToolRequest {
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionRequest,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct FunctionRequest {
    name: String,
    description: String,
    parameters: Option<ToolInputSchema>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct CompletionDeltaResponse {
    content: Option<String>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct CompletionChoiceResponse {
    delta: CompletionDeltaResponse,
    finish_reason: Option<String>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct CompletionResponse {
    id: String,
    choices: Vec<CompletionChoiceResponse>,
    usage: Option<CompletionUsageResponse>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
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
    fn from(msg: &Message) -> Self {
        Self {
            role: if msg.is_context() {
                "system".to_string()
            } else if msg.is_system() {
                "assistant".to_string()
            } else {
                "user".to_string()
            },
            content: msg.text().to_string(),
        }
    }
}
