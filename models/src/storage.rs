use crate::Conversation;

#[derive(Debug, Clone, Default)]
pub struct FilterConversation {
    id: Option<String>,
    title: Option<String>,
    message_contains: Option<String>,
    updated_at_from: Option<chrono::DateTime<chrono::Utc>>,
    updated_at_to: Option<chrono::DateTime<chrono::Utc>>,
    created_at_from: Option<chrono::DateTime<chrono::Utc>>,
    created_at_to: Option<chrono::DateTime<chrono::Utc>>,
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

    pub fn with_updated_at_from(mut self, from: chrono::DateTime<chrono::Utc>) -> Self {
        self.updated_at_from = Some(from);
        self
    }

    pub fn with_updated_at_to(mut self, to: chrono::DateTime<chrono::Utc>) -> Self {
        self.updated_at_to = Some(to);
        self
    }

    pub fn with_created_at_from(mut self, from: chrono::DateTime<chrono::Utc>) -> Self {
        self.created_at_from = Some(from);
        self
    }

    pub fn with_created_at_to(mut self, to: chrono::DateTime<chrono::Utc>) -> Self {
        self.created_at_to = Some(to);
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

    pub fn updated_at_from(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.updated_at_from
    }

    pub fn updated_at_to(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.updated_at_to
    }

    pub fn created_at_from(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.created_at_from
    }

    pub fn created_at_to(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.created_at_to
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

        if let Some(from) = self.updated_at_from {
            if conversation.updated_at() < from {
                return false;
            }
        }

        if let Some(to) = self.updated_at_to {
            if conversation.updated_at() > to {
                return false;
            }
        }

        if let Some(from) = self.created_at_from {
            if conversation.created_at() < from {
                return false;
            }
        }

        if let Some(to) = self.created_at_to {
            if conversation.created_at() > to {
                return false;
            }
        }

        true
    }
}
