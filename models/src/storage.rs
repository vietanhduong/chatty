use crate::Conversation;

#[derive(Debug, Clone, Default)]
pub struct FilterConversation {
    id: Option<String>,
    title: Option<String>,
    message_contains: Option<String>,
    message_issuer: Option<String>,
    start_time: Option<chrono::DateTime<chrono::Utc>>,
    end_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl FilterConversation {
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_message_contains(mut self, message_contains: impl Into<String>) -> Self {
        self.message_contains = Some(message_contains.into());
        self
    }

    pub fn with_message_issuer(mut self, message_issuer: impl Into<String>) -> Self {
        self.message_issuer = Some(message_issuer.into());
        self
    }

    pub fn with_start_time(mut self, start_time: chrono::DateTime<chrono::Utc>) -> Self {
        self.start_time = Some(start_time);
        self
    }

    pub fn with_end_time(mut self, end_time: chrono::DateTime<chrono::Utc>) -> Self {
        self.end_time = Some(end_time);
        self
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn message_contains(&self) -> Option<&str> {
        self.message_contains.as_deref()
    }

    pub fn message_issuer(&self) -> Option<&str> {
        self.message_issuer.as_deref()
    }

    pub fn start_time(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.start_time
    }

    pub fn end_time(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.end_time
    }

    pub fn matches(&self, conversation: &Conversation) -> bool {
        if let Some(id) = &self.id {
            if conversation.id() != id {
                return false;
            }
        }

        if let Some(title) = &self.title {
            if conversation.title() != title {
                return false;
            }
        }

        if let Some(message_contains) = &self.message_contains {
            if !conversation
                .messages()
                .iter()
                .any(|msg| msg.text().contains(message_contains))
            {
                return false;
            }
        }

        if let Some(message_issuer) = &self.message_issuer {
            if !conversation
                .messages()
                .iter()
                .any(|msg| msg.issuer_str() == message_issuer)
            {
                return false;
            }
        }

        if let Some(start_time) = self.start_time {
            if conversation.timestamp() < start_time {
                return false;
            }
        }

        if let Some(end_time) = self.end_time {
            if conversation.timestamp() > end_time {
                return false;
            }
        }

        true
    }
}
