pub(crate) mod migration;

use std::collections::HashMap;

use async_trait::async_trait;
use eyre::{Context, Result, bail};
use openai_models::{Conversation, Message, message::Issuer, storage::FilterConversation};
use tokio_rusqlite::{Connection, OpenFlags, ToSql, named_params, params};

use crate::Storage;

pub struct Sqlite {
    conn: Connection,
}

impl Sqlite {
    pub async fn new(path: Option<&str>) -> Result<Self> {
        let conn = match path {
            Some(path) => Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            )
            .await
            .wrap_err(format!("opening database path: {}", path))?,
            None => Connection::open_in_memory()
                .await
                .wrap_err("opening in-memory database")?,
        };

        let ret = Self { conn };
        ret.run_migration().await.wrap_err("running migration")?;
        Ok(ret)
    }

    async fn run_migration(&self) -> Result<()> {
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
                    "SELECT id, title, context, created_at, updated_at FROM conversations WHERE id = ?",
                )?;
                let mut rows = stmt.query(params![id])?;

                let mut conversation: Option<Conversation> = None;
                if let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    let title: String = row.get(1)?;
                    let context: String = row.get(2)?;
                    let created_at: i64 = row.get(3)?;
                    let created_at= chrono::DateTime::from_timestamp_millis(created_at).ok_or(
                        tokio_rusqlite::Error::Other(eyre::eyre!("invalid created_at value").into()),
                    )?;

                    let updated_at: i64 = row.get(4)?;
                    let updated_at = chrono::DateTime::from_timestamp_millis(updated_at).ok_or(
                        tokio_rusqlite::Error::Other(eyre::eyre!("invalid updated_at value").into()),
                    )?;

                    let mut con = Conversation::default()
                        .with_id(id)
                        .with_title(title)
                        .with_created_at(created_at);
                    if updated_at.timestamp_millis() > 0 {
                        con = con.with_updated_at(updated_at);
                    }

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
            "SELECT id, conversation_id, text, issuer, system, created_at FROM messages WHERE conversation_id = ?",
        )?;

        let mut rows = stmt.query(params![conversation_id])?;
        let mut messages = vec![];
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let text: String = row.get(2)?;
            let issuer: String = row.get(3)?;
            let system: i32 = row.get(4)?;
            let created_at: i64 = row.get(5)?;

            let issuer = if system == 1 {
                Issuer::System(issuer)
            } else {
                Issuer::User(issuer)
            };

            let created_at = chrono::DateTime::from_timestamp_millis(created_at).ok_or(tokio_rusqlite::Error::Other(eyre::eyre!("invalid timestamp").into()))?;

            messages.push(Message::new(issuer, text).with_id(id).with_created_at(created_at));
        }

        Ok(messages)
        }).await?;
        Ok(messages)
    }

    async fn get_conversations(
        &self,
        filter: FilterConversation,
    ) -> Result<HashMap<String, Conversation>> {
        let mut conversations = self
            .conn
            .call(move |conn| {
                let (query, params) = filter_to_query(&filter);
                let mut stmt = conn.prepare(&query)?;
                let params: Vec<(&str, &dyn ToSql)> =
                    params.iter().map(|(n, v)| (*n, v.as_ref())).collect();
                let mut rows = stmt.query(params.as_slice())?;

                let mut conversations: HashMap<String, Conversation> = HashMap::new();

                while let Some(row) = rows.next()? {
                    let id: String = row.get(0)?;
                    let title: String = row.get(1)?;
                    let context: String = row.get(2)?;
                    let created_at: i64 = row.get(3)?;
                    let created_at = chrono::DateTime::from_timestamp_millis(created_at).ok_or(
                        tokio_rusqlite::Error::Other(eyre::eyre!("invalid created_at").into()),
                    )?;

                    let updated_at: i64 = row.get(4)?;
                    let updated_at = chrono::DateTime::from_timestamp_millis(updated_at).ok_or(
                        tokio_rusqlite::Error::Other(eyre::eyre!("invalid updated_at").into()),
                    )?;

                    let mut con = Conversation::default()
                        .with_id(&id)
                        .with_title(title)
                        .with_created_at(created_at);

                    if updated_at.timestamp_millis() > 0 {
                        con = con.with_updated_at(updated_at);
                    }

                    if !context.is_empty() {
                        con.set_context(context);
                    }
                    conversations.insert(id, con);
                }
                Ok(conversations)
            })
            .await?;

        for (_, conversation) in &mut conversations {
            let messages = self.get_messages(conversation.id()).await?;
            conversation.messages_mut().extend(messages);
        }

        Ok(conversations)
    }

    async fn upsert_converstation(&self, conversation: Conversation) -> Result<()> {
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    r#"INSERT INTO conversations (id, title, context, created_at, updated_at)
                VALUES (:id, :title, :context, :created_at, :updated_at)
                ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    context = excluded.context,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at
                "#,
                    named_params! {
                        ":id": conversation.id(),
                        ":title": conversation.title(),
                        ":context": conversation.context().unwrap_or_default(),
                        ":created_at": conversation.created_at().timestamp_millis(),
                        ":updated_at": conversation.updated_at().timestamp_millis(),
                    },
                )?;
                tx.commit()?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    async fn delete_conversation(&self, id: &str) -> Result<()> {
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

    async fn add_messages(
        &self,
        conversation_id: &str,
        messages: &[openai_models::Message],
    ) -> Result<()> {
        let conversation_id = conversation_id.to_string();
        let messages = messages.to_vec();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                for message in messages {
                    tx.execute(
                    r#"INSERT INTO messages (id, conversation_id, text, issuer, system, created_at)
            VALUES (:id, :conversation_id, :text, :issuer, :system, :created_at)
            ON CONFLICT(id, conversation_id) DO UPDATE SET
                text = excluded.text,
                issuer = excluded.issuer,
                system = excluded.system,
                created_at = excluded.created_at
            "#,
                    named_params! {
                        ":id": message.id(),
                        ":conversation_id": conversation_id,
                        ":text": message.text(),
                        ":issuer": message.issuer_str(),
                        ":system": message.is_system() as i32,
                        ":created_at": message.created_at().timestamp_millis()
                    },
                )?;
                }
                Ok(tx.commit()?)
            })
            .await?;
        Ok(())
    }

    async fn upsert_message(&self, conversation_id: &str, message: Message) -> Result<()> {
        let conversation_id = conversation_id.to_string();
        let id = message.id().to_string();
        let text = message.text().to_string();
        let issuer = message.issuer_str().to_string();
        let system = message.is_system() as i32;
        let timestamp = message.created_at().timestamp_millis();
        let affected_rows = self
            .conn
            .call(move |conn| {
                Ok(conn.execute(
                    r#"INSERT INTO messages (id, conversation_id, text, issuer, system, created_at)
            VALUES (:id, :conversation_id, :text, :issuer, :system, :created_at)
            ON CONFLICT(id, conversation_id) DO UPDATE SET
                text = excluded.text,
                issuer = excluded.issuer,
                system = excluded.system,
                created_at = excluded.created_at
            "#,
                    named_params! {
                        ":id": id,
                        ":conversation_id": conversation_id,
                        ":text": text,
                        ":issuer": issuer,
                        ":system": system,
                        ":created_at": timestamp
                    },
                )?)
            })
            .await?;

        if affected_rows == 0 {
            bail!("no rows updated for message with id {}", message.id());
        }
        Ok(())
    }

    async fn delete_messsage(&self, id: &str) -> Result<()> {
        let id = id.to_string();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute("DELETE FROM messages WHERE id = ?", params![id])?;
                Ok(tx.commit()?)
            })
            .await?;
        Ok(())
    }
}

fn filter_to_query(filter: &FilterConversation) -> (String, Vec<(&str, Box<dyn ToSql>)>) {
    let mut query = String::from(
        "SELECT id, title, context, created_at, updated_at FROM conversations WHERE 1=1",
    );
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

    if let Some(from) = filter.updated_at_from() {
        query.push_str(" AND updated_at >= :updated_at_from");
        params.push((":updated_at_from", Box::new(from.timestamp_millis())));
    }

    if let Some(to) = filter.updated_at_to() {
        query.push_str(" AND updated_at <= :updated_at_to");
        params.push((":updated_at_to", Box::new(to.timestamp_millis())));
    }

    if let Some(from) = filter.created_at_from() {
        query.push_str(" AND created_at >= :created_at_from");
        params.push((":created_at_from", Box::new(from.timestamp_millis())));
    }

    if let Some(to) = filter.created_at_to() {
        query.push_str(" AND created_at <= :created_at_to");
        params.push((":created_at_to", Box::new(to.timestamp_millis())));
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
            "SELECT id, title, context, created_at, updated_at FROM conversations WHERE 1=1 AND id = :id"
        );

        assert_eq!(params.len(), 1);
        assert_eq!(params[0].0, ":id");

        filter = filter.with_title("test");
        let (query, params) = filter_to_query(&filter);
        assert_eq!(
            query,
            "SELECT id, title, context, created_at, updated_at FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title"
        );
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].0, ":id");
        assert_eq!(params[1].0, ":title");

        filter = filter.with_message_contains("test");
        let (query, params) = filter_to_query(&filter);
        assert_eq!(
            query,
            "SELECT id, title, context, created_at, updated_at FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title AND EXISTS (SELECT 1 FROM messages WHERE conversation_id = conversations.id AND text LIKE :message_contains)"
        );

        assert_eq!(params.len(), 3);
        assert_eq!(params[0].0, ":id");
        assert_eq!(params[1].0, ":title");
        assert_eq!(params[2].0, ":message_contains");

        filter = filter.with_created_at_from(chrono::Utc::now());
        let (query, params) = filter_to_query(&filter);
        assert_eq!(
            query,
            "SELECT id, title, context, created_at, updated_at FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title AND EXISTS (SELECT 1 FROM messages WHERE conversation_id = conversations.id AND text LIKE :message_contains) AND created_at >= :created_at_from"
        );
        assert_eq!(params.len(), 4);
        assert_eq!(params[0].0, ":id");
        assert_eq!(params[1].0, ":title");
        assert_eq!(params[2].0, ":message_contains");
        assert_eq!(params[3].0, ":created_at_from");

        filter = filter.with_updated_at_to(chrono::Utc::now());
        let (query, params) = filter_to_query(&filter);
        assert_eq!(
            query,
            "SELECT id, title, context, created_at, updated_at FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title AND EXISTS (SELECT 1 FROM messages WHERE conversation_id = conversations.id AND text LIKE :message_contains) AND updated_at <= :updated_at_to AND created_at >= :created_at_from"
        );
        assert_eq!(params.len(), 5);
        assert_eq!(params[0].0, ":id");
        assert_eq!(params[1].0, ":title");
        assert_eq!(params[2].0, ":message_contains");
        assert_eq!(params[3].0, ":updated_at_to");
        assert_eq!(params[4].0, ":created_at_from");
    }

    #[tokio::test]
    async fn test_upsert_conversation() {
        let db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let expected = Conversation::default()
            .with_id("test_id")
            .with_title("Test Conversation")
            .with_context("Test Context")
            .with_created_at(chrono::Utc::now());

        db.upsert_converstation(expected.clone()).await.unwrap();

        let actual = db.get_conversation("test_id").await.unwrap();
        assert!(actual.is_some());

        let actual = actual.unwrap();
        assert_eq!(actual.id(), "test_id");
        assert_eq!(actual.title(), "Test Conversation");
        assert_eq!(actual.context(), Some("Test Context"));
        assert_eq!(
            actual.created_at().timestamp_millis(),
            expected.created_at().timestamp_millis()
        );
        assert_eq!(actual.messages().len(), 0);
    }

    #[tokio::test]
    async fn test_insert_conversation_with_messages() {
        let db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let mut expected = Conversation::default()
            .with_id("test_id")
            .with_title("Test Conversation")
            .with_context("Test Context")
            .with_created_at(chrono::Utc::now());

        db.upsert_converstation(expected.clone()).await.unwrap();

        let actual = db.get_conversation("test_id").await.unwrap();
        assert!(actual.is_some());

        let actual = actual.unwrap();
        assert_eq!(actual.id(), "test_id");
        assert_eq!(actual.title(), "Test Conversation");
        assert_eq!(actual.context(), Some("Test Context"));
        assert_eq!(
            actual.created_at().timestamp_millis(),
            expected.created_at().timestamp_millis()
        );
        assert_eq!(
            actual.updated_at().timestamp_millis(),
            expected.updated_at().timestamp_millis()
        );

        assert_eq!(actual.messages().len(), 0);

        expected.set_title("Updated Title");
        expected.set_context("Updated Context");

        db.upsert_converstation(expected.clone()).await.unwrap();

        let actual = db.get_conversation("test_id").await.unwrap();
        assert!(actual.is_some());
        let actual = actual.unwrap();
        assert_eq!(actual.id(), "test_id");
        assert_eq!(actual.title(), "Updated Title");
        assert_eq!(actual.context(), Some("Updated Context"));
    }

    #[tokio::test]
    async fn test_add_messages() {
        let db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let messages = vec![
            Message::new_system("system", "System message")
                .with_id("msg1")
                .with_created_at(chrono::Utc::now()),
            Message::new_user("user", "User message")
                .with_id("msg2")
                .with_created_at(chrono::Utc::now()),
        ];

        let conversation = Conversation::default()
            .with_id("test_id")
            .with_title("Test Conversation")
            .with_context("Test Context")
            .with_created_at(chrono::Utc::now())
            .with_messages(messages.clone());

        db.upsert_converstation(conversation.clone()).await.unwrap();
        db.add_messages(conversation.id(), conversation.messages())
            .await
            .unwrap();

        let message = Message::new_system("system", "hello")
            .with_id("msg3")
            .with_created_at(chrono::Utc::now());

        db.add_messages(conversation.id(), &vec![message.clone()])
            .await
            .unwrap();

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
            actual.messages()[2].created_at().timestamp_millis(),
            message.created_at().timestamp_millis()
        );
    }

    #[tokio::test]
    async fn test_delete_conversation() {
        let db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let messages = vec![
            Message::new_system("system", "System message")
                .with_id("msg1")
                .with_created_at(chrono::Utc::now()),
            Message::new_user("user", "User message")
                .with_id("msg2")
                .with_created_at(chrono::Utc::now()),
        ];

        let expected = Conversation::default()
            .with_id("test_id")
            .with_title("Test Conversation")
            .with_context("Test Context")
            .with_created_at(chrono::Utc::now())
            .with_messages(messages.clone());

        db.upsert_converstation(expected.clone()).await.unwrap();

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
    async fn test_get_conversations_with_filter() {
        let db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let conversations = fake_converstations();
        for conversation in &conversations {
            db.upsert_converstation(conversation.clone()).await.unwrap();

            db.add_messages(conversation.id(), conversation.messages())
                .await
                .unwrap();
        }

        let filter = FilterConversation::default().with_id("test_id_0");
        let actual = db.get_conversations(filter).await.unwrap();

        let con = actual.get("test_id_0").unwrap();

        assert_eq!(con.id(), "test_id_0");
        assert_eq!(con.title(), "Even Conversation 0");
        assert_eq!(con.context(), Some("Context 0"));

        let filter = FilterConversation::default().with_title("Odd");
        let actual = db.get_conversations(filter).await.unwrap();
        assert_eq!(actual.len(), 5);

        let expected_ids = vec![
            "test_id_1",
            "test_id_3",
            "test_id_5",
            "test_id_7",
            "test_id_9",
        ];

        for (_, id) in expected_ids.iter().enumerate() {
            let con = actual.get(*id).unwrap();
            assert_eq!(con.id(), *id);
        }

        let filter = FilterConversation::default().with_message_contains("System");
        let actual = db.get_conversations(filter).await.unwrap();

        let expected_ids = vec![
            "test_id_0",
            "test_id_2",
            "test_id_4",
            "test_id_6",
            "test_id_8",
        ];

        assert_eq!(actual.len(), 5);
        for (_, id) in expected_ids.iter().enumerate() {
            let con = actual.get(*id).unwrap();
            assert_eq!(con.id(), *id);
        }
    }

    fn fake_converstations() -> Vec<Conversation> {
        let mut conversations = vec![];
        for i in 0..10 {
            let message = if i % 2 == 0 {
                Message::new_system("system", "System message")
                    .with_id(format!("msg1_{}", i))
                    .with_created_at(chrono::Utc::now())
            } else {
                Message::new_user("user", "User message")
                    .with_id(format!("msg2_{}", i))
                    .with_created_at(chrono::Utc::now())
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
                .with_created_at(chrono::Utc::now())
                .with_messages(messages);
            conversations.push(conversation);
        }
        conversations
    }

    #[tokio::test]
    async fn test_upsert_message() {
        let db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();

        let mut message = Message::new_system("system", "System message")
            .with_id("msg1")
            .with_created_at(chrono::Utc::now());

        let conversation = Conversation::default()
            .with_id("test_id")
            .with_title("Test Conversation")
            .with_context("Test Context")
            .with_created_at(chrono::Utc::now())
            .with_messages(vec![message.clone()]);

        db.upsert_converstation(conversation.clone()).await.unwrap();

        db.add_messages(conversation.id(), &[message.clone()])
            .await
            .unwrap();

        message.append(" hello");
        db.upsert_message("test_id", message.clone()).await.unwrap();
        let actual = db.get_messages("test_id").await.unwrap();
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].id(), "msg1");
        assert_eq!(actual[0].text(), "System message hello");

        db.upsert_message("test_id", message.clone().with_id("msg2"))
            .await
            .unwrap();
        let actual = db.get_messages("test_id").await.unwrap();
        assert_eq!(actual.len(), 2);
        assert_eq!(actual[1].id(), "msg2");
    }

    #[tokio::test]
    async fn test_delete_message() {
        let db = Sqlite::new(None).await.unwrap();
        db.run_migration().await.unwrap();
        let mut message = Message::new_system("system", "System message")
            .with_id("msg1")
            .with_created_at(chrono::Utc::now());

        let conversation = Conversation::default()
            .with_id("test_id")
            .with_title("Test Conversation")
            .with_context("Test Context")
            .with_created_at(chrono::Utc::now())
            .with_messages(vec![message.clone()]);

        db.upsert_converstation(conversation.clone()).await.unwrap();

        db.add_messages(conversation.id(), &[message.clone()])
            .await
            .unwrap();

        message.append(" hello");
        db.upsert_message("test_id", message.clone()).await.unwrap();

        let actual = db.get_messages("test_id").await.unwrap();
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].id(), "msg1");
        assert_eq!(actual[0].text(), "System message hello");

        db.delete_messsage("msg1").await.unwrap();
        let actual = db.get_messages("test_id").await.unwrap();
        assert_eq!(actual.len(), 0);
    }
}
