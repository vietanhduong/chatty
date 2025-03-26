pub mod compressor;
pub mod gemini;
pub mod manager;
pub mod openai;

pub use compressor::Compressor;
pub use gemini::Gemini;
pub use manager::Manager;
pub use openai::OpenAI;

#[cfg(test)]
use mockall::{automock, predicate::*};

use crate::models::{ArcEventTx, BackendConfig, BackendKind, BackendPrompt, Model};
use async_trait::async_trait;
use eyre::{Context, Result};
use std::{sync::Arc, time::Duration};

const TITLE_PROMPT: &str = r#"

---
This is initial message. Please give name a title for this conversation.
The title should be placed at the top of the response, in separate line and starts with #"#;

#[async_trait]
#[cfg_attr(test, automock)]
pub trait Backend {
    fn name(&self) -> &str;
    async fn health_check(&self) -> Result<()>;
    async fn list_models(&self, force: bool) -> Result<Vec<Model>>;
    async fn current_model(&self) -> Option<String>;
    async fn set_current_model(&self, model: &str) -> Result<()>;
    async fn get_completion(&self, prompt: BackendPrompt, event_tx: ArcEventTx) -> Result<()>;
}

pub type ArcBackend = Arc<dyn Backend + Send + Sync>;

pub async fn new_manager(config: &BackendConfig) -> Result<ArcBackend> {
    let connections = config
        .connections()
        .iter()
        .filter(|c| c.enabled())
        .collect::<Vec<_>>();
    if connections.is_empty() {
        eyre::bail!("No backend connections configured");
    }

    let mut manager = manager::Manager::default();
    let default_timeout = config.timeout_secs();
    for connection in connections {
        let backend: ArcBackend = match connection.kind() {
            BackendKind::OpenAI => {
                let mut connection = connection.clone();
                if connection.timeout().is_none() && default_timeout.is_some() {
                    connection = connection
                        .with_timeout(Duration::from_secs(default_timeout.unwrap() as u64));
                }
                let openai: OpenAI = (&connection).into();
                Arc::new(openai)
            }
            BackendKind::Gemini => {
                let mut connection = connection.clone();
                if connection.timeout().is_none() && default_timeout.is_some() {
                    connection = connection
                        .with_timeout(Duration::from_secs(default_timeout.unwrap() as u64));
                }
                let gemini: Gemini = (&connection).into();
                Arc::new(gemini)
            }
        };

        let name = backend.name().to_string();
        manager
            .add_connection(backend)
            .await
            .wrap_err(format!("adding connection: {}", name))?;
        println!("  [+] Added backend: {}", name);
        log::debug!("Added backend connection: {}", name);
    }
    Ok(Arc::new(manager))
}

fn user_agent() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let user_agent = format!("chatty/{}", version);
    user_agent
}
