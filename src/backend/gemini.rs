#[cfg(test)]
#[path = "gemini_test.rs"]
mod tests;

use std::{fmt::Display, time};

use crate::{
    config::user_agent,
    models::{
        ArcEventTx, BackendConnection, BackendPrompt, BackendResponse, BackendUsage, Event,
        Message, Model,
    },
};
use async_trait::async_trait;
use eyre::{Context, Result, bail};
use futures::stream::TryStreamExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncBufReadExt;
use tokio::sync::RwLock;
use tokio_util::io::StreamReader;

use crate::backend::{Backend, TITLE_PROMPT};

pub struct Gemini {
    alias: String,
    endpoint: String,
    api_key: Option<String>,
    timeout: Option<time::Duration>,

    want_models: Vec<String>,

    cache_models: RwLock<Vec<Model>>,
    current_model: RwLock<Option<String>>,
}

impl Gemini {
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
}

#[async_trait]
impl Backend for Gemini {
    fn name(&self) -> &str {
        &self.alias
    }

    async fn health_check(&self) -> Result<()> {
        if self.endpoint.is_empty() {
            bail!("endpoint is not set");
        }

        let cached_models = self.cache_models.read().await;
        if !cached_models.is_empty() {
            return Ok(());
        }
        drop(cached_models);

        self.list_models(true).await?;
        Ok(())
    }

    async fn list_models(&self, force: bool) -> Result<Vec<Model>> {
        if !force && self.cache_models.read().await.len() > 0 {
            return Ok(self.cache_models.read().await.clone());
        }

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

        let res = builder.send().await?.json::<ModelListResponse>().await?;

        let all = self.want_models.is_empty();

        let mut models = res
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

        let mut cache = self.cache_models.write().await;
        *cache = models.clone();
        Ok(models)
    }

    async fn current_model(&self) -> Option<String> {
        let model = self.current_model.read().await;
        model.clone()
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
                .find(|m| m.id() == model)
                .ok_or_else(|| eyre::eyre!("model {} not available", model))?
        };
        let mut default_model = self.current_model.write().await;
        *default_model = Some(model.id().to_string());
        Ok(())
    }

    async fn get_completion(&self, prompt: BackendPrompt, event_tx: ArcEventTx) -> Result<()> {
        if self.current_model().await.is_none() && prompt.model().is_none() {
            bail!("no model is set");
        }

        let mut contents: Vec<Content> = vec![];
        if !prompt.context().is_empty() {
            // FIXME: This approach might not be optimized for large contexts
            contents = prompt
                .context()
                .into_iter()
                .map(Content::from)
                .collect::<Vec<_>>();
        }

        let init_conversation = prompt.context().is_empty();
        let content = if init_conversation && !prompt.no_generate_title() {
            format!("{}\n{}", prompt.text(), TITLE_PROMPT)
        } else {
            prompt.text().to_string()
        };

        contents.push(Content {
            role: "user".to_string(),
            parts: vec![ContentParts::Text(content)],
        });

        let model = match prompt.model() {
            Some(model) => model.to_string(),
            None => self.current_model().await.unwrap(),
        };

        let completion_req = CompletionRequest { contents };

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

        let resp = builder
            .json(&completion_req)
            .send()
            .await
            .wrap_err("sending completion request")?;

        if !resp.status().is_success() {
            let http_code = resp.status().as_u16();
            let err: ErrorResponse = resp.json().await.wrap_err("parsing error response")?;
            let mut err = err.error;
            err.http_code = http_code;
            return Err(err.into());
        }

        let stream = resp.bytes_stream().map_err(|e| {
            let err_msg = e.to_string();
            return std::io::Error::new(std::io::ErrorKind::Interrupted, err_msg);
        });

        let mut lines_reader = StreamReader::new(stream).lines();

        let message_id = uuid::Uuid::new_v4().to_string();
        let mut line_buf: Vec<String> = Vec::new();
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
                ContentParts::Text(ref text) => text,
                ContentParts::InlineData(ref blob) => {
                    log::warn!("Received inline data: {:?}", blob);
                    continue;
                }
            };

            if text.is_empty() {
                continue;
            }

            let msg = BackendResponse {
                id: message_id.clone(),
                model: model.clone(),
                text: text.clone(),
                done: false,
                init_conversation,
                usage: None,
            };
            event_tx.send(Event::BackendPromptResponse(msg)).await?;
        }

        let content = process_line_buffer(&line_buf)?;
        line_buf.clear();

        let text = match content.candidates[0].content.parts[0] {
            ContentParts::Text(ref text) => text.clone(),
            ContentParts::InlineData(ref blob) => {
                log::warn!("Received inline data: {:?}", blob);
                String::new()
            }
        };

        let usage = Some(BackendUsage {
            prompt_tokens: content.usage_metadata.prompt_token_count,
            completion_tokens: content.usage_metadata.candidates_token_count,
            total_tokens: content.usage_metadata.total_token_count,
        });

        let msg = BackendResponse {
            id: message_id,
            model: model.clone(),
            text,
            done: true,
            init_conversation,
            usage,
        };
        event_tx.send(Event::BackendPromptResponse(msg)).await?;
        Ok(())
    }
}

fn process_line_buffer(lines: &[String]) -> Result<GenerateContentResponse> {
    let json_raw = lines.join("").trim().to_string();
    let json_raw = json_raw.strip_prefix("[").unwrap_or(&json_raw).trim();
    let json_raw = json_raw.strip_suffix("]").unwrap_or(&json_raw).trim();
    let json_raw = json_raw.strip_suffix(",").unwrap_or(&json_raw).trim();

    let resp: GenerateContentResponse =
        serde_json::from_str(json_raw).wrap_err("unmarshalling response")?;
    Ok(resp)
}

impl Default for Gemini {
    fn default() -> Self {
        Gemini {
            alias: "Gemini".to_string(),
            endpoint: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            api_key: None,
            timeout: None,

            want_models: Vec::new(),

            cache_models: RwLock::new(Vec::new()),
            current_model: RwLock::new(None),
        }
    }
}

impl From<&BackendConnection> for Gemini {
    fn from(value: &BackendConnection) -> Self {
        let mut backend = Gemini::default();

        backend.alias = value.alias().unwrap_or("Gemini").to_string();
        backend.endpoint = value.endpoint().to_string();

        if let Some(key) = value.api_key() {
            backend.api_key = Some(key.to_string());
        }

        if let Some(timeout) = value.timeout() {
            backend.timeout = Some(timeout);
        }

        backend.with_want_models(value.models().to_vec())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelResponse {
    name: String,
    supported_generation_methods: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ModelListResponse {
    models: Vec<ModelResponse>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentPartsBlob {
    mime_type: String,
    data: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ContentParts {
    Text(String),
    InlineData(ContentPartsBlob),
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Content {
    role: String,
    parts: Vec<ContentParts>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CompletionRequest {
    contents: Vec<Content>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateContentResponse {
    candidates: Vec<GenerateCandidate>,
    usage_metadata: GenerateUsageMetadata,
    model_version: String,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateCandidate {
    content: Content,
    finish_reason: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        write!(f, "OpenAI error ({}): {}", self.http_code, self.message)
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
