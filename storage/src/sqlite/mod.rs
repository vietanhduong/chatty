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

    async fn create_conversation(&mut self, conversation: Conversation) -> Result<()> {
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
                        ":converstation_id": conversation.id(),
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
            "INSERT INTO messages (id, conversation_id, text, issuer, system, timestamp) VALUES (:id, :conversation_id, :text, :issuer, :system, :timestamp)",
            named_params!{
                ":id": message.id(),
                ":conversation_id": conversation_id,
                ":text": message.text(),
                ":isser": message.issuer_str(),
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
