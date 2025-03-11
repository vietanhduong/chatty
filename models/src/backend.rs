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
    pub context: Option<String>,
}

pub struct BackendPrompt {
    text: String,
    context: String,
    regenerate: bool,
}

impl BackendPrompt {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            context: String::new(),
            regenerate: false,
        }
    }

    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.context = ctx.into();
        self
    }

    pub fn with_regenerate(mut self) -> Self {
        self.regenerate = true;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn context(&self) -> &str {
        &self.context
    }

    pub fn regenerate(&self) -> bool {
        self.regenerate
    }
}
