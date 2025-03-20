#[cfg(test)]
#[path = "gemini_test.rs"]
mod tests;

use std::{fmt::Display, time};

use async_trait::async_trait;
use eyre::{Context, Result, bail};
use futures::stream::TryStreamExt;
use openai_models::{BackendConnection, BackendPrompt, Event};
use openai_models::{BackendResponse, Message};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncBufReadExt;
use tokio::sync::{RwLock, mpsc};
use tokio_util::io::StreamReader;

use crate::{Backend, TITLE_PROMPT};

pub struct Gemini {
    alias: String,
    endpoint: String,
    api_key: Option<String>,
    timeout: Option<time::Duration>,

    want_models: Vec<String>,

    cache_models: RwLock<Vec<String>>,
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

    async fn list_models(&self, force: bool) -> Result<Vec<String>> {
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

        let mut builder = reqwest::Client::new().get(url);

        if let Some(timeout) = &self.timeout {
            builder = builder.timeout(*timeout);
        }

        let res = builder.send().await?.json::<ModelListResponse>().await?;

        let all = self.want_models.is_empty();

        let mut models: Vec<String> = res
            .models
            .into_iter()
            .filter(|m| {
                m.supported_generation_methods
                    .contains(&"generateContent".to_string())
                    && (all || self.want_models.contains(&m.name))
            })
            .map(|m| {
                m.name
                    .strip_prefix("models/")
                    .unwrap_or(&m.name)
                    .to_string()
            })
            .collect();

        models.sort();

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

        let mut contents: Vec<Content> = vec![];
        if !prompt.context().is_empty() {
            // FIXME: This approach might not be optimized for large contexts
            contents = prompt
                .context()
                .into_iter()
                .map(Content::from)
                .collect::<Vec<_>>();
        }

        // If user wants to regenerate the prompt, we need to rebuild the context
        // by remove the last assistant message until we find the last user message
        if prompt.regenerate() && !contents.is_empty() {
            let mut i = contents.len() as i32 - 1;
            while i >= 0 {
                if contents[i as usize].role == "user" {
                    break;
                }
                contents.pop();
                i -= 1;
            }
            // Pop the last user message, we will add it again
            contents.pop();
        }

        let init_conversation = prompt.context().is_empty();
        let content = if init_conversation {
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

        let mut builder = reqwest::Client::new().post(url);

        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }

        log::debug!("Sending completion request: {:?}", completion_req);

        let res = builder
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

        let stream = res.bytes_stream().map_err(|e| {
            let err_msg = e.to_string();
            return std::io::Error::new(std::io::ErrorKind::Interrupted, err_msg);
        });

        let mut lines_reader = StreamReader::new(stream).lines();

        let mut last_message = String::new();
        let message_id = uuid::Uuid::new_v4().to_string();
        while let Ok(line) = lines_reader.next_line().await {
            if line.is_none() {
                break;
            }

            let cleaned_line = line.unwrap().trim().to_string();
            if !cleaned_line.starts_with("\"text\": ") {
                continue;
            }

            let content: GenerateContentResponse =
                serde_json::from_str(&format!("{{ {} }}", cleaned_line))
                    .wrap_err("unmarshalling response")?;

            if content.text.is_empty() || content.text == "\n" {
                break;
            }

            last_message += &content.text;
            let msg = BackendResponse {
                id: message_id.clone(),
                model: model.clone(),
                text: content.text.clone(),
                done: false,
                init_conversation,
            };
            event_tx.send(Event::BackendPromptResponse(msg))?;
        }

        let msg = BackendResponse {
            id: message_id,
            model: model.clone(),
            text: String::new(),
            done: true,
            init_conversation,
        };
        event_tx.send(Event::BackendPromptResponse(msg))?;
        Ok(())
    }
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
struct Model {
    name: String,
    supported_generation_methods: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ModelListResponse {
    models: Vec<Model>,
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
    text: String,
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
