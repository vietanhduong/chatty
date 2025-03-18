#[derive(Debug, Clone)]
pub enum Issuer {
    System(String),
    User(String),
}

#[derive(Debug, Clone)]
pub struct Message {
    id: String,
    issuer: Issuer,
    text: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl Message {
    pub fn new(issuer: Issuer, text: impl Into<String>) -> Self {
        Self {
            id: chrono::Utc::now().timestamp().to_string(),
            issuer,
            text: text.into(),
            created_at: chrono::Utc::now(),
        }
    }

    pub fn new_system(system: &str, text: impl Into<String>) -> Self {
        Self {
            id: chrono::Utc::now().timestamp().to_string(),
            issuer: Issuer::System(system.to_string()),
            text: text.into(),
            created_at: chrono::Utc::now(),
        }
    }

    pub fn new_user(user: &str, text: impl Into<String>) -> Self {
        Self {
            id: chrono::Utc::now().timestamp().to_string(),
            issuer: Issuer::User(user.to_string()),
            text: text.into(),
            created_at: chrono::Utc::now(),
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn with_created_at(mut self, timestamp: chrono::DateTime<chrono::Utc>) -> Self {
        self.created_at = timestamp;
        self
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn id(&self) -> &str {
        &self.id
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

    pub fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
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
