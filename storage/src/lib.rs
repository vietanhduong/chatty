pub mod sqlite;

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use eyre::Result;
use openai_models::{
    Context, Conversation, Message,
    config::{Configuration, StorageConfig},
    storage::FilterConversation,
};
use sqlite::Sqlite;

#[async_trait]
pub trait Storage {
    async fn get_conversation(&self, id: &str) -> Result<Option<Conversation>>;
    async fn get_conversations(
        &self,
        filter: FilterConversation,
    ) -> Result<HashMap<String, Conversation>>;
    async fn get_messages(&self, conversation_id: &str) -> Result<Vec<Message>>;
    async fn upsert_conversation(&self, conversation: Conversation) -> Result<()>;
    async fn delete_conversation(&self, id: &str) -> Result<()>;
    async fn add_messages(
        &self,
        conversation_id: &str,
        message: &[openai_models::Message],
    ) -> Result<()>;
    async fn upsert_message(&self, conversation_id: &str, message: Message) -> Result<()>;
    async fn delete_messsage(&self, id: &str) -> Result<()>;
    async fn upsert_context(&self, conversation_id: &str, context: Context) -> Result<()>;
}

pub type ArcStorage = Arc<dyn Storage + Send + Sync>;

pub async fn new_storage(config: &Configuration) -> Result<ArcStorage> {
    let storage_config = config.storage().cloned().unwrap_or_default();

    let storage = match storage_config {
        StorageConfig::Sqlite(sqlite_config) => Arc::new(Sqlite::new(sqlite_config.path()).await?),
    };
    Ok(storage)
}
