pub mod gemini;
pub mod manager;
pub mod openai;

pub use gemini::Gemini;
pub use manager::Manager;
pub use openai::OpenAI;

use async_trait::async_trait;
use eyre::{Context, Result};
use openai_models::{BackendKind, BackendPrompt, Event, config::BackendConfig};
use std::sync::Arc;
use tokio::sync::mpsc;

const TITLE_PROMPT: &str = r#"

---
Please give a title to the conversation. The title should be placed at the top
of the response, in separate line and starts with #"#;

#[async_trait]
pub trait Backend {
    fn name(&self) -> &str;
    async fn health_check(&self) -> Result<()>;
    async fn list_models(&self, force: bool) -> Result<Vec<String>>;
    async fn current_model(&self) -> Option<String>;
    async fn set_default_model(&self, model: &str) -> Result<()>;
    async fn get_completion<'a>(
        &self,
        prompt: BackendPrompt,
        event_tx: &'a mpsc::UnboundedSender<Event>,
    ) -> Result<()>;
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
    let default_timeout = config.timeout();
    for connection in connections {
        let backend: ArcBackend = match connection.kind() {
            BackendKind::OpenAI => {
                let mut connection = connection.clone();
                if connection.timeout().is_none() && default_timeout.is_some() {
                    connection = connection.with_timeout(default_timeout.unwrap());
                }
                let openai: OpenAI = (&connection).into();
                Arc::new(openai)
            }
            BackendKind::Gemini => {
                let mut connection = connection.clone();
                if connection.timeout().is_none() && default_timeout.is_some() {
                    connection = connection.with_timeout(default_timeout.unwrap());
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
        log::debug!("Added backend connection: {}", name);
    }
    Ok(Arc::new(manager))
}
