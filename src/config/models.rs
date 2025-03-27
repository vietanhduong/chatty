use eyre::Result;
use serde::{Deserialize, Serialize};

use crate::config::constants::{KEEP_N_MEESAGES, MAX_CONTEXT_LENGTH, MAX_CONVO_LENGTH};
use crate::models::BackendConnection;

use super::CONFIG;
use super::constants::HELLO_MESSAGE;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Configuration {
    #[serde(default)]
    general: General,

    #[serde(default)]
    log: LogConfig,

    #[serde(default)]
    theme: ThemeConfig,

    backend: Option<BackendConfig>,

    #[serde(default)]
    storage: StorageConfig,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct General {
    hello_message: Option<String>,
    show_usage: Option<bool>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LogConfig {
    level: Option<String>,
    filters: Option<Vec<LogFilter>>,
    file: Option<LogFile>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LogFilter {
    module: String,
    level: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LogFile {
    path: String,
    append: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ThemeConfig {
    name: Option<String>,
    folder_path: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BackendConfig {
    default_model: Option<String>,

    context_compression: ContextCompression,
    timeout_secs: Option<u16>,
    connections: Vec<BackendConnection>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ContextCompression {
    enabled: bool,
    max_tokens: usize,
    max_messages: usize,
    keep_n_messages: usize,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct OpenAIBackend {
    endpoint: Option<String>,
    api_key: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum StorageConfig {
    #[serde(rename = "sqlite")]
    Sqlite(SqliteStorage),
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SqliteStorage {
    path: Option<String>,
}

impl Configuration {
    pub fn instance() -> &'static Configuration {
        CONFIG.get().expect("Config not initialized")
    }

    pub fn init(config: Configuration) -> Result<()> {
        CONFIG
            .set(config)
            .map_err(|_| eyre::eyre!("Config already initialized"))?;
        Ok(())
    }

    pub fn log(&self) -> &LogConfig {
        &self.log
    }

    pub fn theme(&self) -> &ThemeConfig {
        &self.theme
    }

    pub fn backend(&self) -> Option<&BackendConfig> {
        self.backend.as_ref()
    }

    pub fn storage(&self) -> &StorageConfig {
        &self.storage
    }

    pub fn general(&self) -> &General {
        &self.general
    }
}

impl General {
    pub fn hello_message(&self) -> Option<&str> {
        self.hello_message.as_deref()
    }

    pub fn show_usage(&self) -> Option<bool> {
        self.show_usage
    }
}

impl LogConfig {
    pub fn level(&self) -> Option<&str> {
        self.level.as_deref()
    }

    pub fn file(&self) -> Option<&LogFile> {
        self.file.as_ref()
    }

    pub fn filters(&self) -> Option<&[LogFilter]> {
        self.filters.as_deref()
    }
}

impl LogFilter {
    pub fn module(&self) -> &str {
        &self.module
    }

    pub fn level(&self) -> Option<&str> {
        self.level.as_deref()
    }
}

impl LogFile {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn append(&self) -> bool {
        self.append
    }
}

impl ThemeConfig {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn folder_path(&self) -> Option<&str> {
        self.folder_path.as_deref()
    }
}

impl BackendConfig {
    pub fn connections(&self) -> &[BackendConnection] {
        &self.connections
    }

    pub fn default_model(&self) -> Option<&str> {
        self.default_model.as_deref()
    }

    pub fn timeout_secs(&self) -> Option<u16> {
        self.timeout_secs
    }

    pub fn context_compression(&self) -> &ContextCompression {
        &self.context_compression
    }
}

impl ContextCompression {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    pub fn max_messages(&self) -> usize {
        self.max_messages
    }

    pub fn keep_n_messages(&self) -> usize {
        self.keep_n_messages
    }
}

impl OpenAIBackend {
    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }
}

impl SqliteStorage {
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            log: LogConfig::default(),
            theme: ThemeConfig::default(),
            backend: Some(BackendConfig::default()),
            storage: StorageConfig::default(),
            general: General::default(),
        }
    }
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: Some("info".to_string()),
            file: None,
            filters: None,
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

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            default_model: None,
            connections: vec![],
            timeout_secs: None,
            context_compression: ContextCompression::default(),
        }
    }
}

impl Default for ContextCompression {
    fn default() -> Self {
        Self {
            enabled: false,
            max_tokens: MAX_CONTEXT_LENGTH,
            max_messages: MAX_CONVO_LENGTH,
            keep_n_messages: KEEP_N_MEESAGES,
        }
    }
}

impl Default for OpenAIBackend {
    fn default() -> Self {
        Self {
            endpoint: Some("https://api.openapi.com".to_string()),
            api_key: None,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self::Sqlite(SqliteStorage::default())
    }
}

impl Default for SqliteStorage {
    fn default() -> Self {
        Self { path: None }
    }
}

impl Default for General {
    fn default() -> Self {
        Self {
            hello_message: Some(HELLO_MESSAGE.to_string()),
            show_usage: None,
        }
    }
}
