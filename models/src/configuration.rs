use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Configuration {
    log: Option<Log>,
    theme: Option<Theme>,
    backend: Option<Backend>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Log {
    level: Option<String>,
    file: Option<LogFile>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LogFile {
    path: String,
    append: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Theme {
    name: Option<String>,
    folder_path: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Backend {
    openai: Option<OpenAI>,
    models: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct OpenAI {
    endpoint: Option<String>,
    api_key: Option<String>,
}

impl Configuration {
    pub fn log(&self) -> Option<&Log> {
        self.log.as_ref()
    }

    pub fn theme(&self) -> Option<&Theme> {
        self.theme.as_ref()
    }

    pub fn backend(&self) -> Option<&Backend> {
        self.backend.as_ref()
    }
}

impl Log {
    pub fn level(&self) -> Option<&str> {
        self.level.as_deref()
    }

    pub fn file(&self) -> Option<&LogFile> {
        self.file.as_ref()
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

impl Theme {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn folder_path(&self) -> Option<&str> {
        self.folder_path.as_deref()
    }
}

impl Backend {
    pub fn openai(&self) -> Option<&OpenAI> {
        self.openai.as_ref()
    }

    pub fn models(&self) -> Option<&[String]> {
        self.models.as_deref()
    }
}

impl OpenAI {
    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            log: Some(Log::default()),
            theme: Some(Theme::default()),
            backend: Some(Backend::default()),
        }
    }
}

impl Default for Log {
    fn default() -> Self {
        Self {
            level: Some("info".to_string()),
            file: None,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: Some("base16-ocean.dark".to_string()),
            folder_path: None,
        }
    }
}

impl Default for Backend {
    fn default() -> Self {
        Self {
            openai: Some(OpenAI::default()),
            models: None,
        }
    }
}

impl Default for OpenAI {
    fn default() -> Self {
        Self {
            endpoint: Some("https://api.openapi.com".to_string()),
            api_key: None,
        }
    }
}
