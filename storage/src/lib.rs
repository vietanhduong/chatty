pub mod sqlite;

use async_trait::async_trait;
use eyre::Result;
use openai_models::{Conversation, Message, storage::FilterConversation};

#[async_trait]
pub trait Storage {
    async fn get_conversation(&self, id: &str) -> Result<Option<Conversation>>;
    async fn get_conversations(&self, filter: FilterConversation) -> Result<Vec<Conversation>>;
    async fn get_messages(&self, conversation_id: &str) -> Result<Vec<Message>>;
    async fn create_conversation(&mut self, conversation: Conversation) -> Result<()>;
    async fn update_conversation(&mut self, conversation: &Conversation) -> Result<()>;
    async fn delete_conversation(&mut self, id: &str) -> Result<()>;
    async fn add_message(
        &mut self,
        conversation_id: &str,
        message: &openai_models::Message,
    ) -> Result<()>;
}
