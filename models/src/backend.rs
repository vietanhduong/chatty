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

    pub fn append_chat_context(&mut self, code_ctx: Option<CodeContext>) {
        if let Some(ctx) = code_ctx {
            let system_prompt = format!(
                "\nThe coding language is {}. Please add language to any code blocks.",
                ctx.language
            );
            self.text += &system_prompt;

            if !ctx.code.is_empty() {
                let code_prompt = format!("\nThe code is the following:\n```\n{}\n```", ctx.code);
                self.text += &code_prompt
            }
        } else {
            self.text += "\nPlease add language to any code blocks."
        }
    }
}
