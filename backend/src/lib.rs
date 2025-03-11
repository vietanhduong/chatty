pub mod openai;

use std::sync::Arc;

pub use crate::openai::OpenAI;

use async_trait::async_trait;
use eyre::Result;
use openai_models::{BackendPrompt, Event, config::Configuration};
use tokio::sync::{Mutex, mpsc};

#[async_trait]
pub trait Backend {
    async fn health_check(&self) -> Result<()>;
    async fn list_models(&self) -> Result<Vec<String>>;
    fn current_model(&self) -> &str;
    async fn set_model(&mut self, model: &str) -> Result<()>;
    async fn get_completion<'a>(
        &self,
        prompt: BackendPrompt,
        event_tx: &'a mpsc::UnboundedSender<Event>,
    ) -> Result<()>;
}

pub type ArcBackend = Arc<Mutex<dyn Backend + Send + Sync>>;

pub fn new_boxed_backend(config: &Configuration) -> Result<ArcBackend> {
    let backend = config
        .backend()
        .ok_or_else(|| eyre::eyre!("No backend configuration found"))?;

    let openai = backend
        .openai()
        .ok_or_else(|| eyre::eyre!("No OpenAI configuration found"))?;

    let endpoint = openai.endpoint().unwrap_or_default();

    let mut backend = OpenAI::default().with_endpoint(endpoint);

    if let Some(api_key) = openai.api_key() {
        backend = backend.with_token(api_key);
    }
    Ok(Arc::new(Mutex::new(backend)))
}
