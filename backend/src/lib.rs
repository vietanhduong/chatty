pub mod openai;

pub use crate::openai::OpenAI;

use async_trait::async_trait;
use eyre::Result;
use openai_models::{BackendPrompt, Event};
use tokio::sync::mpsc;

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

pub type BoxedBackend = Box<dyn Backend + Send + Sync>;
