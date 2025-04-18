#[cfg(test)]
#[path = "openai_test.rs"]
mod tests;

use crate::backend::mcp::{Tool, ToolInputSchema};
use crate::backend::utils::context_truncation;
use crate::backend::{ArcBackend, Backend, TITLE_PROMPT};
use crate::config::{self, ModelSetting, user_agent};
use crate::models::{
    ArcEventTx, BackendConnection, BackendPrompt, BackendResponse, BackendUsage, Event, Message,
    Model,
};
use crate::{info_event, warn_event};
use async_trait::async_trait;
use eyre::{Context, Result, bail};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
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
    mcp: Option<Arc<dyn mcp::McpClient>>,

    want_models: Vec<String>,
    model_settings: HashMap<String, ModelSetting>,

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

        self.chat_completion(None, init_conversation, prompt.model(), &messages, event_tx)
            .await?;
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

    pub async fn init(&mut self) -> Result<()> {
        let models = self.list_models().await.wrap_err("listing models")?;
        for settings in &config::instance().backend.model_settings {
            let re = settings.model.build().wrap_err("building model filter")?;
            if let Some(model) = models.iter().find(|m| re.is_match(m.id())) {
                self.model_settings
                    .insert(model.id().to_string(), settings.clone());
            }
        }
        Ok(())
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

    pub fn with_mcp(mut self, mcp: Arc<dyn mcp::McpClient>) -> Self {
        self.mcp = Some(mcp);
        self
    }

    async fn get_mcp_tools(&self, event_tx: ArcEventTx) -> Vec<Tool> {
        if let Some(mcp) = &self.mcp {
            let tools = match mcp.list_tools().await {
                Ok(tools) => tools,
                Err(e) => {
                    let _ = event_tx
                        .send(warn_event!(format!("Unable to list tools: {}", e)))
                        .await;
                    return vec![];
                }
            };
            return tools;
        }
        vec![]
    }

    async fn chat_completion(
        &self,
        override_id: Option<String>,
        init_conversation: bool,
        model: &str,
        messages: &[MessageRequest],
        event_tx: ArcEventTx,
    ) -> Result<()> {
        let settings = self.model_settings.get(model);

        let enable_mcp = if let Some(settings) = settings {
            settings.enable_mcp.unwrap_or(true)
        } else {
            true
        };

        let tools = if enable_mcp {
            self.get_mcp_tools(event_tx.clone()).await
        } else {
            vec![]
        };

        let completion_req = CompletionRequest {
            model: model.to_string(),
            messages: messages.to_vec(),
            stream: true,
            max_completion_tokens: self.max_output_tokens,
            tool_choice: if !tools.is_empty() {
                Some("auto".to_string())
            } else {
                None
            },
            tools: tools.iter().map(ToolRequest::from).collect(),
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
            log::error!("Error response: {}", resp);
            let err = serde_json::from_str::<ErrorResponse>(&resp)
                .wrap_err(format!("parsing error response: {}", resp))?;
            let mut err = err.error;
            err.http_code = http_code;
            return Err(err.into());
        }

        let stream = res.bytes_stream().map_err(|e| {
            let err_msg = e.to_string();
            std::io::Error::new(std::io::ErrorKind::Interrupted, err_msg)
        });

        let mut line_readers = StreamReader::new(stream).lines();

        let mut message_id = override_id.unwrap_or_default();
        let mut usage: Option<BackendUsage> = None;

        let mut call_tools: BTreeMap<usize, ToolCallResponse> = BTreeMap::new();

        let mut current_message = MessageRequest {
            role: "assistant".to_string(),
            content: String::new(),
            tool_call_id: None,
            ..Default::default()
        };

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

            let c = match data.choices.first() {
                Some(c) => c,
                None => continue,
            };

            if message_id.is_empty() {
                message_id = data.id;
            }

            c.delta.tool_calls.iter().for_each(|e| {
                if let Some(tool) = call_tools.get_mut(&e.index) {
                    tool.function
                        .arguments
                        .as_mut()
                        .unwrap()
                        .push_str(e.function.arguments.as_deref().unwrap_or_default());
                    return;
                }
                call_tools.insert(e.index, e.clone());
            });

            let text = match c.delta.content {
                Some(ref text) => text.deref().to_string(),
                None => continue,
            };

            current_message.content.push_str(&text);

            event_tx
                .send(Event::ChatCompletionResponse(
                    BackendResponse::new(&message_id, model)
                        .with_text(&text)
                        .with_init_conversation(init_conversation),
                ))
                .await?;

            if call_tools.is_empty() {
                if let Some(usage_data) = data.usage {
                    usage = Some(BackendUsage {
                        prompt_tokens: usage_data.prompt_tokens,
                        completion_tokens: usage_data.completion_tokens,
                        total_tokens: usage_data.total_tokens,
                    });
                }
            }
        }

        if call_tools.is_empty() {
            let mut msg = BackendResponse::new(&message_id, model)
                .with_done()
                .with_init_conversation(init_conversation);
            if let Some(usage) = usage {
                msg = msg.with_usage(usage);
            }
            event_tx.send(Event::ChatCompletionResponse(msg)).await?;
            return Ok(());
        }

        event_tx
            .send(Event::ChatCompletionResponse(
                BackendResponse::new(&message_id, model)
                    .with_text("\n")
                    .with_init_conversation(init_conversation),
            ))
            .await?;

        let call_tools = call_tools.into_values().collect::<Vec<_>>();
        // If there are any tool calls, we need to send them to the MCP
        // for processing
        let tool_call_messages = self
            .call_tool(&call_tools, &tools, event_tx.clone())
            .await
            .wrap_err("calling tools")?;
        let mut messages = messages.to_vec();
        current_message.tool_calls = call_tools;
        messages.push(current_message);
        messages.extend(tool_call_messages);

        Box::pin(self.chat_completion(
            Some(message_id),
            init_conversation,
            model,
            &messages,
            event_tx,
        ))
        .await?;
        Ok(())
    }

    async fn call_tool(
        &self,
        calls: &[ToolCallResponse],
        tools: &[Tool],
        event_tx: ArcEventTx,
    ) -> Result<Vec<MessageRequest>> {
        if self.mcp.is_none() {
            bail!("MCP is not set");
        }
        let mut results = vec![];

        let notice_on_call = config::instance()
            .backend
            .mcp
            .notice_on_call_tool
            .unwrap_or_default();

        for call in calls {
            let args = match call.function.arguments.as_ref() {
                Some(args) => Some(
                    serde_json::from_str::<Value>(args).wrap_err("parsing tool call arguments")?,
                ),
                _ => None,
            };

            let tool_name = call
                .function
                .name
                .as_ref()
                .ok_or_else(|| eyre::eyre!("missing tool name"))?;
            if notice_on_call {
                let provider = match tools.iter().find(|t| &t.name == tool_name) {
                    Some(t) => t.provider.clone(),
                    None => "unknown".to_string(),
                };

                event_tx
                    .send(info_event!(format!(
                        "Calling tool \"{}\" (provider: {})",
                        tool_name, provider
                    )))
                    .await?;
            }

            // TODO: should we log the full description of the tool?
            log::debug!("Calling tool {} with args: {:?}", tool_name, args);

            let resp = self
                .mcp
                .as_ref()
                .unwrap()
                .clone()
                .call_tool(call.function.name.as_ref().unwrap(), args)
                .await
                .wrap_err("calling tool")?;
            let result =
                serde_json::to_string(&resp.content).wrap_err("serializing tool result")?;
            results.push(MessageRequest {
                role: "tool".to_string(),
                content: result,
                tool_call_id: call.id.clone(),
                ..Default::default()
            });
        }
        Ok(results)
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
            model_settings: HashMap::new(),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tool_calls: Vec<ToolCallResponse>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct CompletionRequest {
    model: String,
    messages: Vec<MessageRequest>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    parameters: ToolInputSchema,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct CompletionDeltaResponse {
    content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tool_calls: Vec<ToolCallResponse>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct CompletionChoiceResponse {
    delta: CompletionDeltaResponse,
    finish_reason: Option<String>,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
struct ToolCallResponse {
    index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    tool_type: Option<String>,
    function: FunctionResponse,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
struct FunctionResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<String>,
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
            tool_call_id: None,
            tool_calls: vec![],
        }
    }
}

impl From<&Tool> for ToolRequest {
    fn from(tool: &Tool) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: FunctionRequest {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.input_schema.clone(),
            },
        }
    }
}
