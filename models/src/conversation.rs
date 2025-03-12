use uuid::Uuid;

use crate::Message;

#[derive(Debug, Clone)]
pub struct Conversation {
    id: String,
    title: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    messages: Vec<Message>,
    context: Option<String>,
}

impl Conversation {
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn with_timestamp(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        self.timestamp = timestamp;
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

    pub fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        self.timestamp
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
}

impl Default for Conversation {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: "New Chat".to_string(),
            messages: Vec::new(),
            timestamp: chrono::Utc::now(),
            context: None,
        }
    }
}
