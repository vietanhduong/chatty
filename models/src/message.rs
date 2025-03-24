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
    token_count: usize,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl Message {
    pub fn new(issuer: Issuer, text: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            issuer,
            text: text.into(),
            token_count: 0,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn new_system(system: &str, text: impl Into<String>) -> Self {
        Self::new(Issuer::System(system.to_string()), text)
    }

    pub fn new_user(user: &str, text: impl Into<String>) -> Self {
        Self::new(Issuer::User(user.to_string()), text)
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

    pub fn with_token_count(mut self, token_count: usize) -> Self {
        self.set_token_count(token_count);
        self
    }

    pub fn set_token_count(&mut self, token_count: usize) {
        self.token_count = token_count;
    }

    pub fn token_count(&self) -> usize {
        self.token_count
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
}

impl Issuer {
    pub fn user() -> Self {
        Self::User("".to_string())
    }

    pub fn user_with_name(name: impl Into<String>) -> Self {
        Self::User(name.into())
    }

    pub fn system() -> Self {
        Self::System("".to_string())
    }

    pub fn system_with_name(name: impl Into<String>) -> Self {
        Self::System(name.into())
    }
}
