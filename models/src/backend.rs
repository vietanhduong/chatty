#[derive(Default)]
pub struct CodeContext {
    pub language: String,
    pub code: String,
}

pub struct BackendResponse {
    pub model: String,
    pub text: String,
    pub done: bool,
    pub context: Option<String>,
}

pub struct BackendPrompt {
    text: String,
    context: String,
}

impl BackendPrompt {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            context: String::new(),
        }
    }

    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.context = ctx.into();
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn context(&self) -> &str {
        &self.context
    }

    pub fn append_context(&mut self, ctx: impl Into<String>) {
        self.context.push_str(&ctx.into());
    }
}
