use crate::{Message, message::Issuer};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Conversation {
    id: String,
    title: String,
    messages: Vec<Message>,
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

    /// Return a vector of messages. The return vector is alway end up
    /// with a message from system
    pub fn build_context(&self) -> Vec<Message> {
        // If the conversation has less than 3 messages, return an empty vector
        // 1 for hello message and 1 for user message so which means the conversation
        // is not started yet. No context is needed.
        if self.messages.len() < 3 {
            return vec![];
        }
        let mut context = self.messages[1..].to_vec();
        if !context.last().unwrap().is_system() {
            context.pop();
        }
        return context;
    }

    pub fn last_message_of_mut(&mut self, issuer: Option<Issuer>) -> Option<&mut Message> {
        for msg in self.messages.iter_mut().rev() {
            if filter_issuer(issuer.as_ref(), msg) {
                return Some(msg);
            }
        }
        None
    }

    pub fn last_message_of(&self, issuer: Option<Issuer>) -> Option<&Message> {
        for msg in self.messages.iter().rev() {
            if filter_issuer(issuer.as_ref(), msg) {
                return Some(msg);
            }
        }
        None
    }
}

fn filter_issuer(issuer: Option<&Issuer>, msg: &Message) -> bool {
    if issuer.is_none() {
        return true;
    }

    let value;
    let is_system = match issuer.unwrap() {
        Issuer::System(sys) => {
            value = sys.to_string();
            true
        }
        Issuer::User(val) => {
            value = val.to_string();
            false
        }
    };

    if is_system != msg.is_system() {
        return false;
    }

    value.is_empty() || msg.issuer_str() == value
}

impl Default for Conversation {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: "New Chat".to_string(),
            messages: vec![],
            created_at: chrono::Utc::now(),
            updated_at: None,
        }
    }
}
