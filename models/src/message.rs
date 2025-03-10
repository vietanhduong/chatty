#[derive(Debug, Clone)]
pub enum Issuer {
    System(String),
    User(String),
}

#[derive(Debug, Clone)]
pub struct Message {
    issuer: Issuer,
    text: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl Message {
    pub fn new(issuer: Issuer, text: impl Into<String>) -> Self {
        Self {
            issuer,
            text: text.into(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn new_system(system: &str, text: impl Into<String>) -> Self {
        Self {
            issuer: Issuer::System(system.to_string()),
            text: text.into(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn new_user(user: &str, text: impl Into<String>) -> Self {
        Self {
            issuer: Issuer::User(user.to_string()),
            text: text.into(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn is_system(&self) -> bool {
        matches!(self.issuer, Issuer::System(_))
    }

    pub fn issuer(&self) -> &Issuer {
        &self.issuer
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        self.timestamp
    }

    pub fn issuer_str(&self) -> &str {
        match &self.issuer {
            Issuer::System(s) => s,
            Issuer::User(u) => u,
        }
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
}
