use uuid::Uuid;

use crate::Message;

#[derive(Debug, Clone)]
pub struct Converstation {
    id: String,
    title: String,
    messages: Vec<Message>,
}

impl Converstation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
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

impl Default for Converstation {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: "New Chat".to_string(),
            messages: Vec::new(),
        }
    }
}
