use uuid::Uuid;

use crate::{Message, message::Issuer};

#[derive(Debug, Clone)]
pub struct Conversation {
    id: String,
    title: String,
    messages: Vec<Message>,
    context: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Conversation {
    pub fn new_hello() -> Self {
        let mut conversation = Self::default();
        conversation.messages.push(Message::new_system(
            "system",
            "Hello! How can I help you? 😊",
        ));
        conversation
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn with_created_at(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        self.created_at = timestamp;
        if self.updated_at.is_none() {
            self.updated_at = Some(timestamp);
        }
        self
    }

    pub fn with_updated_at(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        self.updated_at = Some(timestamp);
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn set_context(&mut self, context: impl Into<String>) {
        self.context = Some(context.into());
    }

    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    pub fn set_updated_at(&mut self, timestamp: chrono::DateTime<chrono::Utc>) {
        self.updated_at = Some(timestamp);
    }

    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.updated_at.unwrap_or(self.created_at)
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn last_message(&self) -> Option<&Message> {
        self.messages.last()
    }

    pub fn last_mut_message(&mut self) -> Option<&mut Message> {
        self.messages.last_mut()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    pub fn last_message_of(&self, issuer: Option<Issuer>) -> Option<&Message> {
        if let Some(msg) = self.messages.last() {
            if issuer.is_none() {
                return Some(msg);
            }

            let value;
            let is_system = match msg.issuer() {
                Issuer::System(sys) => {
                    value = sys.to_string();
                    true
                }
                Issuer::User(val) => {
                    value = val.to_string();
                    false
                }
            };

            if is_system == msg.is_system() {
                if value.is_empty() {
                    return Some(msg);
                }
                return if msg.issuer_str() == value {
                    Some(msg)
                } else {
                    None
                };
            }
        }
        None
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: "New Chat".to_string(),
            messages: vec![],
            created_at: chrono::Utc::now(),
            updated_at: None,
            context: None,
        }
    }
}
