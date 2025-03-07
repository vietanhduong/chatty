use openai_models::Message;

#[derive(Default)]
pub struct CodeBlocks {
    codeblocks: Vec<String>,
}

impl CodeBlocks {
    pub fn replace_from_messages(&mut self, messages: &[Message]) {
        self.codeblocks = messages.iter().flat_map(|m| m.codeblocks()).collect();
    }
}
