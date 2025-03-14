pub mod openai;

use std::sync::Arc;

pub use crate::openai::OpenAI;

use async_trait::async_trait;
use eyre::Result;
use openai_models::{BackendPrompt, Event, config::Configuration};
use tokio::sync::mpsc;

#[async_trait]
pub trait Backend {
    async fn health_check(&self) -> Result<()>;
    async fn list_models(&self, force: bool) -> Result<Vec<String>>;
    fn default_model(&self) -> String;
    async fn set_default_model(&self, model: &str) -> Result<()>;
    async fn get_completion<'a>(
        &self,
        prompt: BackendPrompt,
        event_tx: &'a mpsc::UnboundedSender<Event>,
    ) -> Result<()>;
}

pub type ArcBackend = Arc<dyn Backend + Send + Sync>;

pub fn new_backend(config: &Configuration) -> Result<ArcBackend> {
    let backend = config
        .backend()
        .ok_or_else(|| eyre::eyre!("No backend configuration found"))?;

    let openai = backend
        .openai()
        .ok_or_else(|| eyre::eyre!("No OpenAI configuration found"))?;

    let endpoint = openai.endpoint().unwrap_or("https://api.openai.com");

    let mut backend = OpenAI::default().with_endpoint(endpoint);

    if let Some(api_key) = openai.api_key() {
        backend = backend.with_token(api_key);
    }
    Ok(Arc::new(backend))
}
