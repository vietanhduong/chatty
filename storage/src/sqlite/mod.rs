pub(crate) mod migration;

use async_trait::async_trait;
use eyre::{Context, Result};
use openai_models::{Conversation, Message, message::Issuer, storage::FilterConversation};
use tokio_rusqlite::{Connection, ToSql, named_params, params};

use crate::Storage;

pub struct Sqlite {
    conn: Connection,
}

impl Sqlite {
    pub async fn new(path: Option<&str>) -> Result<Self> {
        let conn = match path {
            Some(path) => Connection::open(path)
                .await
                .wrap_err(format!("opening database path: {}", path))?,
            None => Connection::open_in_memory()
                .await
                .wrap_err("opening in-memory database")?,
        };

        Ok(Self { conn })
    }

    pub async fn run_migration(&self) -> Result<()> {
        self.conn
            .call(|conn| Ok(conn.execute_batch(migration::MIGRATION)?))
            .await
            .wrap_err("executing migration")?;
        Ok(())
    }
}

#[async_trait]
impl Storage for Sqlite {
    async fn get_conversation(&self, id: &str) -> Result<Option<Conversation>> {
        let id = id.to_string();
        let conversation = self
            .conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, title, context, timestamp FROM conversations WHERE id = ?",
                )?;
                let mut rows = stmt.query(params![id])?;

                let mut conversation: Option<Conversation> = None;
                if let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    let title: String = row.get(1)?;
                    let context: String = row.get(2)?;
                    let timestamp: i64 = row.get(3)?;
                    let timestamp = chrono::DateTime::from_timestamp_millis(timestamp).ok_or(
                        tokio_rusqlite::Error::Other(eyre::eyre!("invalid timestamp").into()),
                    )?;

                    let mut con = Conversation::default()
                        .with_id(id)
                        .with_title(title)
                        .with_timestamp(timestamp);

                    if !context.is_empty() {
                        con.set_context(context);
                    }
                    conversation = Some(con);
                };
                Ok(conversation)
            })
            .await?;

        if conversation.is_none() {
            return Ok(None);
        }

        let conversation = conversation.unwrap();
        let messages = self.get_messages(conversation.id()).await?;

        Ok(Some(conversation.with_messages(messages)))
    }

    async fn get_messages(&self, conversation_id: &str) -> Result<Vec<Message>> {
        let conversation_id = conversation_id.to_string();
        let messages = self.conn.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, text, issuer, system, timestamp FROM messages WHERE conversation_id = ?",
        )?;

        let mut rows = stmt.query(params![conversation_id])?;
        let mut messages = vec![];
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let text: String = row.get(2)?;
            let issuer: String = row.get(3)?;
            let system: i32 = row.get(4)?;
            let timestamp: i64 = row.get(5)?;

            let issuer = if system == 1 {
                Issuer::System(issuer)
            } else {
                Issuer::User(issuer)
            };

            let timestamp = chrono::DateTime::from_timestamp_millis(timestamp).ok_or(tokio_rusqlite::Error::Other(eyre::eyre!("invalid timestamp").into()))?;

            messages.push(Message::new(issuer, text).with_id(id).with_timestamp(timestamp));
        }

        Ok(messages)
        }).await?;
        Ok(messages)
    }

    async fn get_conversations(&self, filter: FilterConversation) -> Result<Vec<Conversation>> {
        let mut conversations = self
            .conn
            .call(move |conn| {
                let (query, params) = filter_to_query(&filter);
                let mut stmt = conn.prepare(&query)?;
                let params: Vec<(&str, &dyn ToSql)> =
                    params.iter().map(|(n, v)| (*n, v.as_ref())).collect();
                let mut rows = stmt.query(params.as_slice())?;

                let mut conversations = vec![];
                while let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    let title: String = row.get(1)?;
                    let context: String = row.get(2)?;
                    let timestamp: i64 = row.get(3)?;
                    let timestamp = chrono::DateTime::from_timestamp_millis(timestamp).ok_or(
                        tokio_rusqlite::Error::Other(eyre::eyre!("invalid timestamp").into()),
                    )?;

                    let mut con = Conversation::default()
                        .with_id(id)
                        .with_title(title)
                        .with_timestamp(timestamp);
                    if !context.is_empty() {
                        con.set_context(context);
                    }
                    conversations.push(con);
                }
                Ok(conversations)
            })
            .await?;

        for conversation in &mut conversations {
            let messages = self.get_messages(conversation.id()).await?;
            conversation.messages_mut().extend(messages);
        }

        Ok(conversations)
    }

    async fn insert_conversation(&mut self, conversation: Conversation) -> Result<()> {
        self.conn.call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO conversations (id, title, context, timestamp) VALUES (:id, :title, :context, :timestamp)",
                named_params!{
                    ":id": conversation.id(),
                    ":title": conversation.title(),
                    ":context": conversation.context().unwrap_or_default(),
                    ":timestamp": conversation.timestamp().timestamp_millis()
                },
            )?;

            for message in conversation.messages() {
                tx.execute(
                    "INSERT INTO messages (id, conversation_id, text, issuer, system, timestamp) VALUES (:id, :conversation_id, :text, :issuer, :system, :timestamp)",
                    named_params!{
                        ":id": message.id(),
                        ":conversation_id": conversation.id(),
                        ":text": message.text(),
                        ":issuer": message.issuer_str(),
                        ":system": message.is_system() as i32,
                        ":timestamp": message.timestamp().timestamp_millis()
                    },
                )?;
            }
            tx.commit()?;
            Ok(())
        }).await?;
        Ok(())
    }

    async fn update_conversation(&mut self, conversation: &Conversation) -> Result<()> {
        let id = conversation.id().to_string();
        let title = conversation.title().to_string();
        let context = conversation.context().unwrap_or_default().to_string();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "UPDATE conversations SET title = :title, context = :context WHERE id = :id",
                    named_params! {
                        ":id": id,
                        ":title": title,
                        ":context": context
                    },
                )?;
                Ok(tx.commit()?)
            })
            .await?;
        Ok(())
    }

    async fn delete_conversation(&mut self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute("DELETE FROM conversations WHERE id = ?", params![id])?;
                Ok(tx.commit()?)
            })
            .await?;
        Ok(())
    }

    async fn add_message(
        &mut self,
        conversation_id: &str,
        message: &openai_models::Message,
    ) -> Result<()> {
        let conversation_id = conversation_id.to_string();
        let message = message.clone();
        self.conn.call(move |conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO messages (id, conversation_id, text, issuer, system, timestamp) VALUES (:id, :conversation_id, :text, :issuer, :system, :timestamp)",
            named_params!{
                ":id": message.id(),
                ":conversation_id": conversation_id,
                ":text": message.text(),
                ":issuer": message.issuer_str(),
                ":system": message.is_system() as i32,
                ":timestamp": message.timestamp().timestamp_millis()
            },
        )?;
        Ok(tx.commit()?)
        }).await?;
        Ok(())
    }
}

fn filter_to_query(filter: &FilterConversation) -> (String, Vec<(&str, Box<dyn ToSql>)>) {
    let mut query =
        String::from("SELECT id, title, context, timestamp FROM conversations WHERE 1=1");
    let mut params: Vec<(&str, Box<dyn ToSql>)> = vec![];

    if let Some(id) = filter.id() {
        query.push_str(" AND id = :id");
        params.push((":id", Box::new(id.to_string())));
    }

    if let Some(title) = filter.title() {
        query.push_str(" AND title LIKE :title");
        params.push((":title", Box::new(format!("%{}%", title))));
    }

    if let Some(message_contains) = filter.message_contains() {
        query.push_str(" AND EXISTS (SELECT 1 FROM messages WHERE conversation_id = conversations.id AND text LIKE :message_contains)");
        params.push((
            ":message_contains",
            Box::new(format!("%{}%", message_contains)),
        ));
    }

    if let Some(start_time) = filter.start_time() {
        query.push_str(" AND timestamp >= :start_time");
        params.push((":start_time", Box::new(start_time.timestamp_millis())));
    }

    if let Some(end_time) = filter.end_time() {
        query.push_str(" AND timestamp <= :end_time");
        params.push((":end_time", Box::new(end_time.timestamp_millis())));
    }

    (query, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_to_query() {
        let mut filter = FilterConversation::default().with_id("test_id");

        let (query, params) = filter_to_query(&filter);
        assert_eq!(
            query,
            "SELECT id, title, context, timestamp FROM conversations WHERE 1=1 AND id = :id"
        );

        assert_eq!(params.len(), 1);
        assert_eq!(params[0].0, ":id");

        filter = filter.with_title("test");
        let (query, params) = filter_to_query(&filter);
        assert_eq!(
            query,
            "SELECT id, title, context, timestamp FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title"
        );
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].0, ":id");
        assert_eq!(params[1].0, ":title");

        filter = filter.with_message_contains("test");
        let (query, params) = filter_to_query(&filter);
        assert_eq!(
            query,
            "SELECT id, title, context, timestamp FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title AND EXISTS (SELECT 1 FROM messages WHERE conversation_id = conversations.id AND text LIKE :message_contains)"
        );

        assert_eq!(params.len(), 3);
        assert_eq!(params[0].0, ":id");
        assert_eq!(params[1].0, ":title");
        assert_eq!(params[2].0, ":message_contains");

        filter = filter.with_start_time(chrono::Utc::now());
        let (query, params) = filter_to_query(&filter);
        assert_eq!(
            query,
            "SELECT id, title, context, timestamp FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title AND EXISTS (SELECT 1 FROM messages WHERE conversation_id = conversations.id AND text LIKE :message_contains) AND timestamp >= :start_time"
        );
        assert_eq!(params.len(), 4);
        assert_eq!(params[0].0, ":id");
        assert_eq!(params[1].0, ":title");
        assert_eq!(params[2].0, ":message_contains");
        assert_eq!(params[3].0, ":start_time");

        filter = filter.with_end_time(chrono::Utc::now());
        let (query, params) = filter_to_query(&filter);
        assert_eq!(
            query,
            "SELECT id, title, context, timestamp FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title AND EXISTS (SELECT 1 FROM messages WHERE conversation_id = conversations.id AND text LIKE :message_contains) AND timestamp >= :start_time AND timestamp <= :end_time"
        );
        assert_eq!(params.len(), 5);
        assert_eq!(params[0].0, ":id");
        assert_eq!(params[1].0, ":title");
        assert_eq!(params[2].0, ":message_contains");
        assert_eq!(params[3].0, ":start_time");
        assert_eq!(params[4].0, ":end_time");
    }

    #[tokio::test]
    async fn test_insert_conversation() {
        let mut db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let expected = Conversation::default()
            .with_id("test_id")
            .with_title("Test Conversation")
            .with_context("Test Context")
            .with_timestamp(chrono::Utc::now());

        db.insert_conversation(expected.clone()).await.unwrap();

        let actual = db.get_conversation("test_id").await.unwrap();
        assert!(actual.is_some());

        let actual = actual.unwrap();
        assert_eq!(actual.id(), "test_id");
        assert_eq!(actual.title(), "Test Conversation");
        assert_eq!(actual.context(), Some("Test Context"));
        assert_eq!(
            actual.timestamp().timestamp_millis(),
            expected.timestamp().timestamp_millis()
        );
        assert_eq!(actual.messages().len(), 0);
    }

    #[tokio::test]
    async fn test_insert_conversation_with_messages() {
        let mut db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let messages = vec![
            Message::new_system("system", "System message")
                .with_id("msg1")
                .with_timestamp(chrono::Utc::now()),
            Message::new_user("user", "User message")
                .with_id("msg2")
                .with_timestamp(chrono::Utc::now()),
        ];

        let expected = Conversation::default()
            .with_id("test_id")
            .with_title("Test Conversation")
            .with_context("Test Context")
            .with_timestamp(chrono::Utc::now())
            .with_messages(messages.clone());

        db.insert_conversation(expected.clone()).await.unwrap();

        let actual = db.get_conversation("test_id").await.unwrap();
        assert!(actual.is_some());

        let actual = actual.unwrap();
        assert_eq!(actual.id(), "test_id");
        assert_eq!(actual.title(), "Test Conversation");
        assert_eq!(actual.context(), Some("Test Context"));
        assert_eq!(
            actual.timestamp().timestamp_millis(),
            expected.timestamp().timestamp_millis()
        );
        assert_eq!(actual.messages().len(), 2);
        assert_eq!(actual.messages()[0].id(), "msg1");
        assert_eq!(actual.messages()[1].id(), "msg2");
        assert_eq!(actual.messages()[0].text(), "System message");
        assert_eq!(actual.messages()[1].text(), "User message");
        assert_eq!(actual.messages()[0].issuer_str(), "system");
        assert_eq!(actual.messages()[1].issuer_str(), "user");
        assert_eq!(actual.messages()[0].is_system(), true);
        assert_eq!(actual.messages()[1].is_system(), false);
        assert_eq!(
            actual.messages()[0].timestamp().timestamp_millis(),
            messages[0].timestamp().timestamp_millis()
        );
        assert_eq!(
            actual.messages()[1].timestamp().timestamp_millis(),
            messages[1].timestamp().timestamp_millis()
        );
    }

    #[tokio::test]
    async fn test_add_message() {
        let mut db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let messages = vec![
            Message::new_system("system", "System message")
                .with_id("msg1")
                .with_timestamp(chrono::Utc::now()),
            Message::new_user("user", "User message")
                .with_id("msg2")
                .with_timestamp(chrono::Utc::now()),
        ];

        let conversation = Conversation::default()
            .with_id("test_id")
            .with_title("Test Conversation")
            .with_context("Test Context")
            .with_timestamp(chrono::Utc::now())
            .with_messages(messages.clone());

        db.insert_conversation(conversation.clone()).await.unwrap();

        let message = Message::new_system("system", "hello")
            .with_id("msg3")
            .with_timestamp(chrono::Utc::now());

        db.add_message(conversation.id(), &message).await.unwrap();

        let actual = db.get_conversation(conversation.id()).await.unwrap();
        assert!(actual.is_some());

        let actual = actual.unwrap();
        assert_eq!(actual.id(), conversation.id());
        assert_eq!(actual.messages().len(), 3);
        assert_eq!(actual.messages()[2].id(), "msg3");
        assert_eq!(actual.messages()[2].text(), "hello");
        assert_eq!(actual.messages()[2].issuer_str(), "system");
        assert_eq!(actual.messages()[2].is_system(), true);
        assert_eq!(
            actual.messages()[2].timestamp().timestamp_millis(),
            message.timestamp().timestamp_millis()
        );
    }

    #[tokio::test]
    async fn test_update_conversation() {
        let mut db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let conversation = Conversation::default()
            .with_id("test_id")
            .with_title("Test Conversation")
            .with_context("Test Context")
            .with_timestamp(chrono::Utc::now());

        db.insert_conversation(conversation.clone()).await.unwrap();

        let updated_conversation = conversation
            .clone()
            .with_title("Updated Title")
            .with_context("Updated Context");

        db.update_conversation(&updated_conversation).await.unwrap();

        let actual = db.get_conversation("test_id").await.unwrap();
        assert!(actual.is_some());

        let actual = actual.unwrap();
        assert_eq!(actual.id(), "test_id");
        assert_eq!(actual.title(), "Updated Title");
        assert_eq!(actual.context(), Some("Updated Context"));
        assert_eq!(
            actual.timestamp().timestamp_millis(),
            conversation.timestamp().timestamp_millis()
        );

        let updated_conversation = conversation
            .clone()
            .with_title("Updated Title")
            .with_context("");

        db.update_conversation(&updated_conversation).await.unwrap();
        let actual = db.get_conversation("test_id").await.unwrap();
        assert!(actual.is_some());
        let actual = actual.unwrap();
        assert_eq!(actual.id(), "test_id");
        assert_eq!(actual.title(), "Updated Title");
        assert_eq!(actual.context(), None);
        assert_eq!(
            actual.timestamp().timestamp_millis(),
            conversation.timestamp().timestamp_millis()
        );
    }

    #[tokio::test]
    async fn test_delete_conversation() {
        let mut db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let messages = vec![
            Message::new_system("system", "System message")
                .with_id("msg1")
                .with_timestamp(chrono::Utc::now()),
            Message::new_user("user", "User message")
                .with_id("msg2")
                .with_timestamp(chrono::Utc::now()),
        ];

        let expected = Conversation::default()
            .with_id("test_id")
            .with_title("Test Conversation")
            .with_context("Test Context")
            .with_timestamp(chrono::Utc::now())
            .with_messages(messages.clone());

        db.insert_conversation(expected.clone()).await.unwrap();

        let actual = db.get_conversation("test_id").await.unwrap();
        assert!(actual.is_some());

        let actual = actual.unwrap();
        assert_eq!(actual.id(), "test_id");

        db.delete_conversation("test_id").await.unwrap();
        let actual = db.get_conversation("test_id").await.unwrap();
        assert!(actual.is_none());

        let actual = db.get_messages("test_id").await.unwrap();
        assert!(actual.is_empty());
    }

    #[tokio::test]
    async fn test_get_conversation_not_exist() {
        let db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let actual = db.get_conversation("non_existent_id").await.unwrap();
        assert!(actual.is_none());
    }

    #[tokio::test]
    async fn test_get_conversation_with_filter() {
        let mut db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let conversations = fake_converstations();
        for conversation in &conversations {
            db.insert_conversation(conversation.clone()).await.unwrap();
        }

        let filter = FilterConversation::default().with_id("test_id_0");
        let actual = db.get_conversations(filter).await.unwrap();
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].id(), "test_id_0");
        assert_eq!(actual[0].title(), "Even Conversation 0");
        assert_eq!(actual[0].context(), Some("Context 0"));
        assert_eq!(actual[0].messages().len(), 3);

        let filter = FilterConversation::default().with_title("Odd");
        let actual = db.get_conversations(filter).await.unwrap();
        assert_eq!(actual.len(), 5);
        assert_eq!(actual[0].id(), "test_id_1");
        assert_eq!(actual[1].id(), "test_id_3");
        assert_eq!(actual[2].id(), "test_id_5");
        assert_eq!(actual[3].id(), "test_id_7");
        assert_eq!(actual[4].id(), "test_id_9");

        let filter = FilterConversation::default().with_message_contains("System");
        let actual = db.get_conversations(filter).await.unwrap();
        assert_eq!(actual.len(), 5);
        assert_eq!(actual[0].id(), "test_id_0");
        assert_eq!(actual[1].id(), "test_id_2");
        assert_eq!(actual[2].id(), "test_id_4");
        assert_eq!(actual[3].id(), "test_id_6");
        assert_eq!(actual[4].id(), "test_id_8");
    }

    fn fake_converstations() -> Vec<Conversation> {
        let mut conversations = vec![];
        for i in 0..10 {
            let message = if i % 2 == 0 {
                Message::new_system("system", "System message")
                    .with_id(format!("msg1_{}", i))
                    .with_timestamp(chrono::Utc::now())
            } else {
                Message::new_user("user", "User message")
                    .with_id(format!("msg2_{}", i))
                    .with_timestamp(chrono::Utc::now())
            };

            let messages = vec![
                message.clone(),
                message.clone().with_id(format!("msg3_{}", i)),
                message.clone().with_id(format!("msg4_{}", i)),
            ];

            let title = if i % 2 == 0 {
                "Even Conversation"
            } else {
                "Odd Conversation"
            };

            let conversation = Conversation::default()
                .with_id(format!("test_id_{}", i))
                .with_title(format!("{} {}", title, i))
                .with_context(format!("Context {}", i))
                .with_timestamp(chrono::Utc::now())
                .with_messages(messages);
            conversations.push(conversation);
        }
        conversations
    }
}
