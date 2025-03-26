use crate::models::Message;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, time};

#[derive(Default)]
pub struct CodeContext {
    pub language: String,
    pub code: String,
}

#[derive(Debug)]
pub struct BackendResponse {
    pub model: String,
    pub id: String,
    pub text: String,
    pub done: bool,
    pub init_conversation: bool,
    pub usage: Option<BackendUsage>,
}

#[derive(Debug, Default, Clone)]
pub struct BackendUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

pub struct BackendPrompt {
    model: Option<String>,
    text: String,
    context: Vec<Message>,
    no_generate_title: bool,
}

impl BackendPrompt {
    pub fn new(text: impl Into<String>) -> BackendPrompt {
        BackendPrompt {
            model: None,
            text: text.into(),
            context: vec![],
            no_generate_title: false,
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = Some(model.to_string());
        self
    }

    pub fn with_context(mut self, ctx: Vec<Message>) -> Self {
        self.context = ctx;
        self
    }

    pub fn with_no_generate_title(mut self) -> Self {
        self.no_generate_title = true;
        self
    }

    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn context(&self) -> &[Message] {
        &self.context
    }

    pub fn no_generate_title(&self) -> bool {
        self.no_generate_title
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BackendConnection {
    enabled: bool,
    kind: BackendKind,
    alias: Option<String>,
    endpoint: String,
    api_key: Option<String>,
    timeout: Option<time::Duration>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    models: Vec<String>,
}

impl BackendConnection {
    pub fn new(kind: BackendKind, endpoint: impl Into<String>) -> Self {
        Self {
            enabled: false,
            kind,
            alias: None,
            endpoint: endpoint.into(),
            api_key: None,
            timeout: None,
            models: Vec::new(),
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.models = models;
        self
    }

    pub fn add_model(mut self, model: String) -> Self {
        self.models.push(model);
        self
    }

    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.alias = Some(alias.into());
        self
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_timeout(mut self, timeout: impl Into<time::Duration>) -> Self {
        self.timeout = Some(timeout.into());
        self
    }

    pub fn kind(&self) -> &BackendKind {
        &self.kind
    }

    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
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

    pub fn models(&self) -> &[String] {
        &self.models
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Hash, PartialEq, Eq, Deserialize, Serialize, Debug, Clone)]
pub enum BackendKind {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "gemini")]
    Gemini,
}

impl Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendKind::OpenAI => write!(f, "open_ai"),
            BackendKind::Gemini => write!(f, "gemini"),
        }
    }
}

impl Display for BackendUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Prompt Tokens: {}, Completion Token: {}, Total: {}",
            self.prompt_tokens, self.completion_tokens, self.total_tokens
        )
    }
}
