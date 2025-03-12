pub(crate) mod migration;

use async_trait::async_trait;
use eyre::{Context, Result, bail};
use openai_models::{Conversation, Message, message::Issuer, storage::FilterConversation};
use tokio_rusqlite::{Connection, params};

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
                let mut stmt =
                    conn.prepare("SELECT id, title, timestamp FROM conversations WHERE id = ?")?;
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

    async fn get_conversations(&self, filter: &FilterConversation) -> Result<Vec<Conversation>> {
        bail!("get_conversations not implemented")
    }

    async fn create_conversation(&mut self, conversation: Conversation) -> Result<()> {
        self.conn.call(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO conversations (id, title, timestamp) VALUES (?, ?, ?)",
                params![
                    conversation.id(),
                    conversation.title(),
                    conversation.timestamp().timestamp_millis()
                ],
            )?;

            for message in conversation.messages() {
                tx.execute(
                    "INSERT INTO messages (id, conversation_id, text, issuer, system, timestamp) VALUES (?, ?, ?, ?, ?, ?)",
                    params![
                        message.id(),
                        conversation.id(),
                        message.text(),
                        message.issuer_str(),
                        message.is_system() as i32,
                        message.timestamp().timestamp_millis()
                    ],
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
                    "UPDATE conversations SET title = ?, context = ? WHERE id = ?",
                    params![title, context, id],
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
            "INSERT INTO messages (id, conversation_id, text, issuer, system, timestamp) VALUES (?, ?, ?, ?, ?, ?)",
            params![
                message.id(),
                conversation_id,
                message.text(),
                message.issuer_str(),
                message.is_system() as i32,
                message.timestamp().timestamp_millis()
            ],
        )?;
        Ok(tx.commit()?)
        }).await?;
        Ok(())
    }
}
