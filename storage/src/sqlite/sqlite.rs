#[cfg(test)]
#[path = "sqlite_test.rs"]
mod tests;

use std::collections::HashMap;

use async_trait::async_trait;
use eyre::{Context, Result, bail};
use openai_models::{Conversation, Message, message::Issuer, storage::FilterConversation};
use tokio_rusqlite::{Connection, OpenFlags, ToSql, named_params, params};

use crate::Storage;

use super::migration::MIGRATION;

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
            .call(|conn| Ok(conn.execute_batch(MIGRATION)?))
            .await
            .wrap_err("executing migration")?;
        Ok(())
    }
}

#[async_trait]
impl Storage for Sqlite {
    async fn get_conversation(&self, id: &str) -> Result<Option<Conversation>> {
        let id = id.to_string();
        let conversation =
            self.conn
                .call(move |conn| {
                    let mut stmt = conn.prepare(
                        "SELECT id, title, created_at, updated_at FROM conversations WHERE id = ?",
                    )?;
                    let mut rows = stmt.query(params![id])?;

                    let mut conversation: Option<Conversation> = None;
                    if let Some(row) = rows.next()? {
                        let id: String = row.get(0)?;
                        let title: String = row.get(1)?;
                        let created_at: i64 = row.get(2)?;
                        let created_at = chrono::DateTime::from_timestamp_millis(created_at)
                            .ok_or(tokio_rusqlite::Error::Other(
                                eyre::eyre!("invalid created_at value").into(),
                            ))?;

                        let updated_at: i64 = row.get(3)?;
                        let updated_at = chrono::DateTime::from_timestamp_millis(updated_at)
                            .ok_or(tokio_rusqlite::Error::Other(
                                eyre::eyre!("invalid updated_at value").into(),
                            ))?;

                        let mut con = Conversation::default()
                            .with_id(id)
                            .with_title(title)
                            .with_created_at(created_at);
                        if updated_at.timestamp_millis() > 0 {
                            con = con.with_updated_at(updated_at);
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
            "SELECT id, conversation_id, text, issuer, system, token_count, created_at FROM messages WHERE conversation_id = ?",
        )?;

        let mut rows = stmt.query(params![conversation_id])?;
        let mut messages = vec![];
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let text: String = row.get(2)?;
            let issuer: String = row.get(3)?;
            let system: i32 = row.get(4)?;
            let token_count: usize = row.get(5)?;
            let created_at: i64 = row.get(6)?;

            let issuer = if system == 1 {
                Issuer::System(issuer)
            } else {
                Issuer::User(issuer)
            };

            let created_at = chrono::DateTime::from_timestamp_millis(created_at).ok_or(tokio_rusqlite::Error::Other(eyre::eyre!("invalid timestamp").into()))?;

            messages.push(Message::new(issuer, text).with_id(id).with_created_at(created_at).with_token_count(token_count));
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
                    let created_at: i64 = row.get(2)?;
                    let created_at = chrono::DateTime::from_timestamp_millis(created_at).ok_or(
                        tokio_rusqlite::Error::Other(eyre::eyre!("invalid created_at").into()),
                    )?;

                    let updated_at: i64 = row.get(3)?;
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

    async fn upsert_conversation(&self, conversation: Conversation) -> Result<()> {
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    r#"INSERT INTO conversations (id, title, created_at, updated_at)
                VALUES (:id, :title, :created_at, :updated_at)
                ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at
                "#,
                    named_params! {
                        ":id": conversation.id(),
                        ":title": conversation.title(),
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
                    r#"INSERT INTO messages (id, conversation_id, text, issuer, system, token_count, created_at)
            VALUES (:id, :conversation_id, :text, :issuer, :system, :token_count, :created_at)
            ON CONFLICT(id, conversation_id) DO UPDATE SET
                text = excluded.text,
                issuer = excluded.issuer,
                system = excluded.system,
                token_count = excluded.token_count,
                created_at = excluded.created_at
            "#,
                    named_params! {
                        ":id": message.id(),
                        ":conversation_id": conversation_id,
                        ":text": message.text(),
                        ":issuer": message.issuer_str(),
                        ":system": message.is_system() as i32,
                        ":token_count": message.token_count() as i32,
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
        let token_count = message.token_count() as i32;
        let timestamp = message.created_at().timestamp_millis();
        let affected_rows = self
            .conn
            .call(move |conn| {
                Ok(conn.execute(
                    r#"INSERT INTO messages (id, conversation_id, text, issuer, system, token_count, created_at)
            VALUES (:id, :conversation_id, :text, :issuer, :system, :token_count, :created_at)
            ON CONFLICT(id, conversation_id) DO UPDATE SET
                text = excluded.text,
                issuer = excluded.issuer,
                system = excluded.system,
                token_count = excluded.token_count,
                created_at = excluded.created_at
            "#,
                    named_params! {
                        ":id": id,
                        ":conversation_id": conversation_id,
                        ":text": text,
                        ":issuer": issuer,
                        ":system": system,
                        ":token_count":token_count,
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
    let mut query =
        String::from("SELECT id, title, created_at, updated_at FROM conversations WHERE 1=1");
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
