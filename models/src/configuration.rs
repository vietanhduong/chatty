use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Configuration {
    log: Option<LogConfig>,
    theme: Option<ThemeConfig>,
    backend: Option<BackendConfig>,
    storage: Option<StorageConfig>,
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
    openai: Option<OpenAIBackend>,
    default_model: Option<String>,
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
    pub fn log(&self) -> Option<&LogConfig> {
        self.log.as_ref()
    }

    pub fn theme(&self) -> Option<&ThemeConfig> {
        self.theme.as_ref()
    }

    pub fn backend(&self) -> Option<&BackendConfig> {
        self.backend.as_ref()
    }

    pub fn storage(&self) -> Option<&StorageConfig> {
        self.storage.as_ref()
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
    pub fn openai(&self) -> Option<&OpenAIBackend> {
        self.openai.as_ref()
    }

    pub fn default_model(&self) -> Option<&str> {
        self.default_model.as_deref()
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
            log: Some(LogConfig::default()),
            theme: Some(ThemeConfig::default()),
            backend: Some(BackendConfig::default()),
            storage: Some(StorageConfig::default()),
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
            openai: Some(OpenAIBackend::default()),
            default_model: None,
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
