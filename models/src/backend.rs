use crate::{Message, message::Issuer};

#[derive(Default)]
pub struct CodeContext {
    pub language: String,
    pub code: String,
}

#[derive(Debug)]
pub struct BackendResponse {
    pub model: String,
    pub id: String,
    pub text: String,
    pub done: bool,
    pub context: Vec<Message>,
}

pub struct BackendPrompt {
    text: String,
    context: Vec<Message>,
}

impl BackendPrompt {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            context: vec![],
        }
    }

    pub fn with_context(mut self, ctx: impl IntoIterator<Item = Message>) -> Self {
        self.context = ctx.into_iter().collect();
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn context(&self) -> &[Message] {
        &self.context
    }

    pub fn append_context(&mut self, message: &Message) {
        if let Issuer::System(sys) = message.issuer() {
            if sys == "system" {
                return;
            }
        }
        self.context.push(message.clone());
    }
}
