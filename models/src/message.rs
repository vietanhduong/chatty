pub struct Message {
    system: bool,
    text: String,
}

impl Message {
    pub fn new(system: bool, text: impl Into<String>) -> Self {
        Self {
            system,
            text: text.into(),
        }
    }

    pub fn system(&self) -> bool {
        self.system
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn append(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.text += &text.replace('\t', "  ");
    }

    pub fn codeblocks(&self) -> Vec<String> {
        let mut codeblocks: Vec<String> = vec![];
        let mut current_codeblock: Vec<&str> = vec![];
        let mut in_codeblock = false;
        for line in self.text.split('\n') {
            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                if in_codeblock {
                    codeblocks.push(current_codeblock.join("\n"));
                    current_codeblock.clear();
                }
                in_codeblock = !in_codeblock;
                continue;
            }
            if in_codeblock {
                current_codeblock.push(line);
            }
        }

        codeblocks
    }

    pub fn author(&self) -> String {
        if self.system {
            "System".to_string()
        } else {
            "User".to_string()
        }
    }
}
