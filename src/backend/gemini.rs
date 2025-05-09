#[cfg(test)]
#[path = "gemini_test.rs"]
mod tests;

use std::{collections::HashMap, fmt::Display, sync::Arc, time};

use crate::{
    backend::{mcp::Tool, utils::context_truncation},
    config::{self, ModelSetting, user_agent},
    info_event,
    models::{
        ArcEventTx, BackendConnection, BackendPrompt, BackendResponse, BackendUsage, Event,
        Message, Model,
    },
    warn_event,
};
use async_trait::async_trait;
use eyre::{Context, Result, bail};
use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncBufReadExt;
use tokio_util::io::StreamReader;

use crate::backend::{Backend, TITLE_PROMPT};

use super::mcp::{self, ToolInputSchema};

pub struct Gemini {
    alias: String,
    endpoint: String,
    api_key: Option<String>,
    timeout: Option<time::Duration>,
    mcp: Option<Arc<dyn mcp::McpClient>>,

    want_models: Vec<String>,
    max_output_tokens: Option<usize>,

    model_settings: HashMap<String, ModelSetting>,
}

impl Gemini {
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

    pub fn with_want_models(mut self, models: Vec<String>) -> Self {
        self.want_models = models
            .into_iter()
            .map(|s| format_model(s.as_str()))
            .collect();
        self
    }

    pub fn with_alias(mut self, alias: &str) -> Self {
        self.alias = alias.to_string();
        self
    }

    pub fn with_mcp(mut self, mcp: Arc<dyn mcp::McpClient>) -> Self {
        self.mcp = Some(mcp);
        self
    }

    pub fn with_max_output_tokens(mut self, max_output_tokens: usize) -> Self {
        self.max_output_tokens = Some(max_output_tokens);
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
        contents: &[Content],
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
            contents: contents.to_vec(),
            generation_config: Some(GenerationConfig {
                max_output_tokens: self.max_output_tokens,
            }),
            tools: tools.iter().map(ToolRequest::from).collect(),
            tool_config: None,
        };

        let mut params = vec![];
        if let Some(key) = &self.api_key {
            params.push(("key", key));
        }

        let url = reqwest::Url::parse_with_params(
            &format!("{}/models/{}:streamGenerateContent", self.endpoint, model),
            params.as_slice(),
        )
        .wrap_err("parsing url")?;

        let mut builder = reqwest::Client::new()
            .post(url)
            .header("User-Agent", user_agent());

        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }

        log::trace!("Sending completion request: {:?}", completion_req);

        let mut function_calls = vec![];

        let resp = builder
            .json(&completion_req)
            .send()
            .await
            .wrap_err("sending completion request")?;

        if !resp.status().is_success() {
            let http_code = resp.status().as_u16();
            let text = resp.text().await.wrap_err("reading error response")?;

            let err: ErrorResponse = serde_json::from_str(&text)
                .wrap_err(format!("parsing error response: {}", text))?;
            let mut err = err.error;
            err.http_code = http_code;
            return Err(err.into());
        }

        let stream = resp.bytes_stream().map_err(|e| {
            let err_msg = e.to_string();
            std::io::Error::new(std::io::ErrorKind::Interrupted, err_msg)
        });

        let mut lines_reader = StreamReader::new(stream).lines();

        let message_id = override_id.unwrap_or(uuid::Uuid::new_v4().to_string());
        let mut line_buf: Vec<String> = Vec::new();
        let mut completion_text = String::new();
        while let Ok(line) = lines_reader.next_line().await {
            if line.is_none() {
                break;
            }

            let cleaned_line = line.unwrap().trim().to_string();
            log::trace!("Received line: {}", cleaned_line);
            // Gemini separte array object by a line with a comma
            if cleaned_line != "," {
                line_buf.push(cleaned_line);
                continue;
            }

            // Process the line buffer
            let content = process_line_buffer(&line_buf)?;
            line_buf.clear();
            if content.candidates.is_empty() || content.candidates[0].finish_reason.is_some() {
                break;
            }

            let text = match content.candidates[0].content.parts[0] {
                ContentParts::Text(ref text) => {
                    completion_text.push_str(text);
                    text.to_string()
                }
                ContentParts::FunctionCall(ref func_call) => {
                    function_calls.push(func_call.clone());
                    String::new()
                }
                // TODO(vietanhduong): Handle this properly
                _ => String::new(),
            };

            if text.is_empty() {
                continue;
            }

            // event_tx
            //     .send(Event::ChatCompletionResponse(BackendResponse {
            //         id: message_id.clone(),
            //         model: model.to_string(),
            //         text: text.clone(),
            //         done: false,
            //         init_conversation,
            //         usage: None,
            //     }))
            //     .await?;

            event_tx
                .send(Event::ChatCompletionResponse(
                    BackendResponse::new(&message_id, model)
                        .with_text(&text)
                        .with_init_conversation(init_conversation),
                ))
                .await?;
        }

        let content = process_line_buffer(&line_buf)?;
        line_buf.clear();

        let text = match content.candidates[0].content.parts[0] {
            ContentParts::Text(ref text) => text.clone(),
            ContentParts::FunctionCall(ref func_call) => {
                function_calls.push(func_call.clone());
                String::new()
            }
            // TODO(vietanhduong): Handle this properly
            _ => String::new(),
        };

        if function_calls.is_empty() {
            let usage = BackendUsage {
                prompt_tokens: content.usage_metadata.prompt_token_count,
                completion_tokens: content.usage_metadata.candidates_token_count,
                total_tokens: content.usage_metadata.total_token_count,
            };

            event_tx
                .send(Event::ChatCompletionResponse(
                    BackendResponse::new(&message_id, model)
                        .with_done()
                        .with_text(text)
                        .with_init_conversation(init_conversation)
                        .with_usage(usage),
                ))
                .await?;
            return Ok(());
        }

        event_tx
            .send(Event::ChatCompletionResponse(
                BackendResponse::new(&message_id, model)
                    .with_init_conversation(init_conversation)
                    .with_text("\n"),
            ))
            .await?;

        let function_responses = self
            .call_tool(&function_calls, &tools, event_tx.clone())
            .await?;
        let mut contents = contents.to_vec();
        let mut current_content = Content {
            role: "model".to_string(),
            parts: vec![],
        };

        if !text.is_empty() {
            current_content.parts.push(ContentParts::Text(text));
        }

        current_content
            .parts
            .extend(function_calls.into_iter().map(ContentParts::FunctionCall));

        contents.push(current_content);
        contents.push(Content {
            role: "user".to_string(),
            parts: function_responses
                .into_iter()
                .map(ContentParts::FunctionResponse)
                .collect(),
        });

        Box::pin(self.chat_completion(
            Some(message_id),
            init_conversation,
            model,
            &contents,
            event_tx,
        ))
        .await
    }

    async fn call_tool(
        &self,
        calls: &[FunctionCall],
        tools: &[Tool],
        event_tx: ArcEventTx,
    ) -> Result<Vec<FunctionResponse>> {
        if self.mcp.is_none() {
            bail!("MCP is not set");
        }

        let notice_on_call = config::instance()
            .backend
            .mcp
            .notice_on_call_tool
            .unwrap_or_default();

        let mut results = vec![];
        for call in calls {
            let tool_name = call.name.clone();
            if notice_on_call {
                let provider = match tools.iter().find(|t| t.name == tool_name) {
                    Some(tool) => tool.provider.clone(),
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
            log::debug!("Calling tool {} with args: {:?}", tool_name, call.args);

            let resp = self
                .mcp
                .as_ref()
                .unwrap()
                .clone()
                .call_tool(&call.name, call.args.clone())
                .await
                .wrap_err("calling tool")?;
            let mut response = HashMap::new();
            response.insert("result".to_string(), resp.content);
            let result = serde_json::to_value(&response).wrap_err("serializing tool result")?;
            results.push(FunctionResponse {
                id: call.id.clone(),
                name: call.name.clone(),
                response: Some(result),
            });
        }
        Ok(results)
    }
}

#[async_trait]
impl Backend for Gemini {
    fn name(&self) -> &str {
        &self.alias
    }

    async fn list_models(&self) -> Result<Vec<Model>> {
        let mut params = vec![];
        if let Some(key) = &self.api_key {
            params.push(("key", key));
        }

        let url = reqwest::Url::parse_with_params(
            format!("{}/models", &self.endpoint).as_str(),
            params.as_slice(),
        )
        .wrap_err("parsing url")?;

        let mut builder = reqwest::Client::new()
            .get(url)
            .header("User-Agent", user_agent());

        if let Some(timeout) = &self.timeout {
            builder = builder.timeout(*timeout);
        }

        let resp = builder.send().await?;

        if !resp.status().is_success() {
            let http_code = resp.status().as_u16();
            let err: ErrorResponse = resp.json().await.wrap_err("parsing error response")?;
            let mut err = err.error;
            err.http_code = http_code;
            return Err(err.into());
        }

        let all = self.want_models.is_empty();

        let mut models = resp
            .json::<ModelListResponse>()
            .await
            .wrap_err("parsing model list response")?
            .models
            .into_iter()
            .filter(|m| {
                m.supported_generation_methods
                    .contains(&"generateContent".to_string())
                    && (all || self.want_models.contains(&m.name))
            })
            .map(|m| {
                Model::new(
                    m.name
                        .strip_prefix("models/")
                        .unwrap_or(&m.name)
                        .to_string(),
                )
                .with_provider(&self.alias)
            })
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

        let contents = messages
            .into_iter()
            .map(|m| Content::from(&m))
            .collect::<Vec<_>>();

        self.chat_completion(None, init_conversation, prompt.model(), &contents, event_tx)
            .await?;
        Ok(())
    }
}

fn process_line_buffer(lines: &[String]) -> Result<GenerateContentResponse> {
    let json_raw = lines.join("").trim().to_string();
    let json_raw = json_raw.strip_prefix("[").unwrap_or(&json_raw).trim();
    let json_raw = json_raw.strip_suffix("]").unwrap_or(json_raw).trim();
    let json_raw = json_raw.strip_suffix(",").unwrap_or(json_raw).trim();

    let resp: GenerateContentResponse =
        serde_json::from_str(json_raw).wrap_err("unmarshalling response")?;
    Ok(resp)
}

impl Default for Gemini {
    fn default() -> Self {
        Gemini {
            max_output_tokens: None,
            alias: "Gemini".to_string(),
            endpoint: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            mcp: None,
            api_key: None,
            timeout: None,

            model_settings: HashMap::new(),
            want_models: Vec::new(),
        }
    }
}

impl From<&BackendConnection> for Gemini {
    fn from(value: &BackendConnection) -> Self {
        let mut backend = Gemini::default();

        if let Some(alias) = value.alias() {
            backend.alias = alias.to_string();
        }

        backend.endpoint = value.endpoint().to_string();

        if let Some(key) = value.api_key() {
            backend.api_key = Some(key.to_string());
        }

        if let Some(timeout) = value.timeout() {
            backend.timeout = Some(timeout);
        }

        backend.max_output_tokens = value.max_output_tokens();

        backend.with_want_models(value.models().to_vec())
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelResponse {
    name: String,
    supported_generation_methods: Vec<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct ModelListResponse {
    #[serde(default)]
    models: Vec<ModelResponse>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentPartsBlob {
    mime_type: String,
    data: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FunctionCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<serde_json::Value>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FunctionResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    response: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ContentParts {
    Text(String),
    InlineData(ContentPartsBlob),
    FunctionCall(FunctionCall),
    FunctionResponse(FunctionResponse),
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct Content {
    role: String,
    parts: Vec<ContentParts>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompletionRequest {
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ToolRequest>,
    tool_config: Option<serde_json::Value>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    code_execution: Option<serde_json::Value>,
    function_declarations: Vec<FunctionDeclerationRequest>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FunctionDeclerationRequest {
    name: String,
    description: Option<String>,
    parameters: ToolInputSchema,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerationConfig {
    max_output_tokens: Option<usize>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateContentResponse {
    candidates: Vec<GenerateCandidate>,
    usage_metadata: GenerateUsageMetadata,
    model_version: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateCandidate {
    content: Content,
    finish_reason: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateUsageMetadata {
    prompt_token_count: usize,
    #[serde(default)]
    candidates_token_count: usize,
    total_token_count: usize,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct ErrorResponse {
    error: GeminiError,
}

#[derive(Default, Error, Debug, Serialize, Deserialize)]
pub struct GeminiError {
    #[serde(skip)]
    pub http_code: u16,
    pub message: String,
    pub code: Option<u16>,
    pub status: Option<String>,
}

impl Display for GeminiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Gemini error ({}): {}", self.http_code, self.message)
    }
}

impl From<&Message> for Content {
    fn from(value: &Message) -> Self {
        let role = if value.is_system() {
            "model".to_string()
        } else {
            "user".to_string()
        };
        let parts = vec![ContentParts::Text(value.text().to_string())];
        Content { role, parts }
    }
}

fn format_model(model: &str) -> String {
    let model = model.strip_prefix("model/").unwrap_or(model);
    let model = model.strip_prefix("models/").unwrap_or(model);
    format!("models/{}", model)
}

impl From<&Tool> for ToolRequest {
    fn from(tool: &Tool) -> Self {
        Self {
            code_execution: None,
            function_declarations: vec![FunctionDeclerationRequest {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.input_schema.clone(),
            }],
        }
    }
}
