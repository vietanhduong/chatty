pub mod manager;
pub mod openai;

pub use crate::openai::OpenAI;

use async_trait::async_trait;
use eyre::{Context, Result, bail};
use openai_models::{BackendKind, BackendPrompt, Event, config::BackendConfig};
use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait Backend {
    fn name(&self) -> String;
    async fn health_check(&self) -> Result<()>;
    async fn list_models(&self, force: bool) -> Result<Vec<String>>;
    fn default_model(&self) -> Option<String>;
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
        let backend = match connection.kind() {
            BackendKind::OpenAI => {
                let mut connection = connection.clone();
                if connection.timeout().is_none() && default_timeout.is_some() {
                    connection = connection.with_timeout(default_timeout.unwrap());
                }
                let openai: OpenAI = (&connection).into();
                Arc::new(openai)
            }
            _ => bail!("Unsupported backend kind: {}", connection.kind()),
        };
        let name = backend.name();
        manager
            .add_connection(backend)
            .await
            .wrap_err(format!("adding connection: {}", name))?;
        log::debug!("Added backend connection: {}", name);
    }
    Ok(Arc::new(manager))
}
