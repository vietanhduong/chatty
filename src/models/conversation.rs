#[cfg(test)]
#[path = "conversation_test.rs"]
mod tests;

use crate::{
    config::{Configuration, constants::HELLO_MESSAGE},
    models::{Message, message::Issuer},
};

#[derive(Debug, Clone)]
pub struct Conversation {
    id: String,
    title: String,
    messages: Vec<Message>,
    contexts: Vec<Context>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Conversation {
    pub fn new_hello() -> Self {
        let mut conversation = Self::default();
        conversation.messages.push(Message::new_system(
            "system",
            Configuration::instance()
                .general
                .hello_message
                .as_deref()
                .unwrap_or(HELLO_MESSAGE),
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
        self.messages.sort_by(|a, b| {
            a.created_at()
                .partial_cmp(&b.created_at())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self
    }

    pub fn with_context(mut self, context: Vec<Context>) -> Self {
        self.contexts = context;
        self.contexts.sort_by(|a, b| {
            a.created_at()
                .partial_cmp(&b.created_at())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self
    }

    pub fn set_id(&mut self, id: impl Into<String>) {
        self.id = id.into();
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn append_message(&mut self, message: Message) {
        self.messages.push(message);
        self.messages.sort_by(|a, b| {
            a.created_at()
                .partial_cmp(&b.created_at())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.updated_at = Some(chrono::Utc::now());
    }

    pub fn append_context(&mut self, context: Context) {
        self.contexts.push(context);
        self.contexts.sort_by(|a, b| {
            a.created_at()
                .partial_cmp(&b.created_at())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
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

    pub fn contexts_mut(&mut self) -> &mut Vec<Context> {
        &mut self.contexts
    }

    pub fn contexts(&self) -> &[Context] {
        &self.contexts
    }

    /// Return a vector of messages. The return vector is always end up
    /// with a message from system
    pub fn build_context(&self) -> Vec<Message> {
        // If the conversation has less than 3 messages, return an empty vector
        // 1 for hello message and 1 for user message so which means the conversation
        // is not started yet. No context is needed.
        if self.messages.len() < 3 && self.contexts.is_empty() {
            return vec![];
        }

        let mut context: Vec<Message> = self.contexts.iter().map(Message::from).collect();

        match self.contexts.last() {
            Some(ctx) => {
                // Find the index of the last message in the messages and
                // append the next messages to the context
                let last_message_index = self
                    .messages
                    .iter()
                    .position(|msg| msg.id() == ctx.last_message_id())
                    .unwrap_or(self.messages.len() - 2);
                // Append the next messages to the context
                context.extend(self.messages[last_message_index + 1..].to_vec());
            }
            None => context.extend(self.messages[1..].to_vec()),
        }

        if !context.last().unwrap().is_system() {
            context.pop();
        }

        return context;
    }

    /// Calculate the total token count of the conversation.
    /// This function will calculate the token count based on the context (if any)
    /// and the messages started from the last context.
    pub fn token_count(&self) -> usize {
        let last_message_id = self
            .contexts
            .last()
            .map(|ctx| ctx.last_message_id())
            .unwrap_or_default();
        if last_message_id.is_empty() {
            return self.messages.iter().map(|msg| msg.token_count()).sum();
        }

        let tokens: usize = self.contexts.iter().map(|ctx| ctx.token_count()).sum();
        let last_message_index = self
            .messages
            .iter()
            .position(|msg| msg.id() == last_message_id)
            .unwrap_or(self.messages.len() - 1);
        tokens
            + self
                .messages
                .iter()
                .skip(last_message_index + 1)
                .map(|msg| msg.token_count())
                .sum::<usize>()
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self {
            id: "".to_string(),
            title: "New Chat".to_string(),
            messages: vec![],
            contexts: vec![],
            created_at: chrono::Utc::now(),
            updated_at: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Context {
    id: String,
    content: String,
    last_message_id: String,
    token_count: usize,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl Context {
    pub fn new(last_message_id: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            content: String::new(),
            token_count: 0,
            last_message_id: last_message_id.to_string(),
            created_at: chrono::Utc::now(),
        }
    }

    pub fn with_token_count(mut self, token_count: usize) -> Self {
        self.token_count = token_count;
        self
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn with_created_at(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        self.created_at = timestamp;
        self
    }

    pub fn append_content(&mut self, content: impl Into<String>) {
        self.content.push_str(&content.into());
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn last_message_id(&self) -> &str {
        &self.last_message_id
    }

    pub fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
    }

    pub fn token_count(&self) -> usize {
        self.token_count
    }

    pub fn set_token_count(&mut self, token_count: usize) {
        self.token_count = token_count;
    }
}

impl From<&Context> for Message {
    fn from(value: &Context) -> Message {
        Message::new_system("system", &value.content)
            .with_id(&value.id)
            .with_created_at(value.created_at)
            .with_token_count(value.token_count)
            .with_context(true)
    }
}

pub fn filter_issuer(issuer: Option<&Issuer>, msg: &Message) -> bool {
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

pub trait FindMessage {
    fn last_message_of(&self, issuer: Option<Issuer>) -> Option<&Message>;
    fn last_message_of_mut(&mut self, issuer: Option<Issuer>) -> Option<&mut Message>;
}

impl FindMessage for Vec<Message> {
    fn last_message_of(&self, issuer: Option<Issuer>) -> Option<&Message> {
        for msg in self.iter().rev() {
            if filter_issuer(issuer.as_ref(), msg) {
                return Some(msg);
            }
        }
        None
    }

    fn last_message_of_mut(&mut self, issuer: Option<Issuer>) -> Option<&mut Message> {
        for msg in self.iter_mut().rev() {
            if filter_issuer(issuer.as_ref(), msg) {
                return Some(msg);
            }
        }
        None
    }
}

impl FindMessage for Conversation {
    fn last_message_of(&self, issuer: Option<Issuer>) -> Option<&Message> {
        self.messages.last_message_of(issuer)
    }

    fn last_message_of_mut(&mut self, issuer: Option<Issuer>) -> Option<&mut Message> {
        self.messages.last_message_of_mut(issuer)
    }
}
