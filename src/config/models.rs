use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::constants::{KEEP_N_MESSAGES, MAX_CONTEXT_LENGTH, MAX_CONVO_LENGTH};
use crate::models::BackendConnection;

#[allow(unused_imports)]
use super::CONFIG;
use super::model_filter::ModelFilter;

use super::constants::{
    HELLO_MESSAGE, LOG_FILE_PATH, MAX_BUBBLE_WIDTH_PERCENT, MIN_BUBBLE_WIDTH_PERCENT,
};
use super::defaults::*;

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct Configuration {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub log: LogConfig,

    #[serde(default)]
    pub theme: ThemeConfig,

    #[serde(default)]
    pub backend: BackendConfig,

    #[serde(default)]
    pub storage: StorageConfig,

    #[serde(default)]
    pub context: ContextConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GeneralConfig {
    #[serde(default = "hello_message")]
    pub hello_message: Option<String>,

    #[serde(default)]
    pub show_usage: Option<bool>,

    #[serde(default)]
    pub verbose: bool,

    #[serde(default = "bubble_width_percent")]
    pub bubble_width_percent: usize,

    #[serde(default)]
    pub auto_start: Option<bool>,

    #[serde(default = "default_option_true")]
    pub bubble: Option<bool>,

    #[serde(default = "default_option_true")]
    pub show_wrapped_indicator: Option<bool>,

    #[serde(default)]
    pub copy_on_select: Option<bool>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone)]
pub struct ContextConfig {
    #[serde(default)]
    pub compression: ContextCompression,

    #[serde(default)]
    pub truncation: TokenTruncation,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ContextCompression {
    #[serde(default)]
    pub enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub compress_model: Option<String>,

    #[serde(default = "max_context_length")]
    pub max_tokens: usize,

    #[serde(default = "max_convo_length")]
    pub max_messages: usize,

    #[serde(default = "keep_n_messages")]
    pub keep_n_messages: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TokenTruncation {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "max_context_length")]
    pub max_tokens: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LogConfig {
    #[serde(default = "log_level")]
    pub level: Option<String>,

    #[serde(default)]
    pub filters: Option<Vec<LogFilter>>,

    #[serde(default)]
    pub file: LogFile,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LogFilter {
    #[serde(default)]
    pub module: Option<String>,

    #[serde(default)]
    pub level: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LogFile {
    #[serde(default = "log_file_path")]
    pub path: String,

    #[serde(default)]
    pub append: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ThemeConfig {
    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub folder_path: Option<String>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone)]
pub struct BackendConfig {
    #[serde(default)]
    pub default_model: Option<String>,

    #[serde(default)]
    pub timeout_secs: Option<u16>,

    #[serde(default)]
    pub connections: Vec<BackendConnection>,

    #[serde(default)]
    pub mcp: McpConfig,

    #[serde(default)]
    pub model_settings: Vec<ModelSetting>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone)]
pub struct McpConfig {
    #[serde(default)]
    pub notice_on_call_tool: Option<bool>,

    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ModelSetting {
    pub model: ModelFilter,
    #[serde(default = "default_option_true")]
    pub enable_mcp: Option<bool>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum McpServer {
    #[serde(rename = "binary")]
    Binary(BinaryConfig),
    #[serde(rename = "websocket")]
    WebSocket(WebSocketConfig),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct McpServerConfig {
    #[serde(default = "default_option_true")]
    pub enabled: Option<bool>,
    pub provider: String,
    #[serde(flatten)]
    pub server: McpServer,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BinaryConfig {
    pub filename: String,

    #[serde(default)]
    pub args: Vec<String>,

    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct WebSocketConfig {
    pub url: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum StorageConfig {
    #[serde(rename = "sqlite")]
    Sqlite(SqliteStorage),
}

#[derive(Default, Deserialize, Serialize, Debug, Clone)]
pub struct SqliteStorage {
    pub path: Option<String>,
}

impl GeneralConfig {
    pub fn get_bubble_width_percent(&self) -> usize {
        let percent = self.bubble_width_percent.min(MAX_BUBBLE_WIDTH_PERCENT);
        percent.max(MIN_BUBBLE_WIDTH_PERCENT)
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: Some("info".to_string()),
            file: LogFile::default(),
            filters: None,
        }
    }
}

impl Default for LogFile {
    fn default() -> Self {
        Self {
            path: LOG_FILE_PATH.to_string(),
            append: false,
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: Some("base16-ocean.dark".to_string()),
            folder_path: None,
        }
    }
}

impl Default for ContextCompression {
    fn default() -> Self {
        Self {
            enabled: false,
            compress_model: None,
            max_tokens: MAX_CONTEXT_LENGTH,
            max_messages: MAX_CONVO_LENGTH,
            keep_n_messages: KEEP_N_MESSAGES,
        }
    }
}

impl Default for TokenTruncation {
    fn default() -> Self {
        Self {
            enabled: false,
            max_tokens: MAX_CONTEXT_LENGTH,
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            hello_message: Some(HELLO_MESSAGE.to_string()),
            show_usage: None,
            bubble_width_percent: 80,
            auto_start: None,
            bubble: default_option_true(),
            show_wrapped_indicator: default_option_true(),
            copy_on_select: None,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self::Sqlite(SqliteStorage::default())
    }
}
