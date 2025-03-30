pub mod backend;
pub mod conversation;
pub mod event;
pub mod message;
pub mod storage;

use std::time;

use ratatui::style::Color;

pub use backend::*;
pub use conversation::{Context, Conversation};
pub use message::Message;

pub use event::{ArcEventTx, Event, EventTx};

pub enum Action {
    CopyMessages(Vec<Message>),
}

#[derive(Debug, Clone)]
pub struct UpsertMessage {
    pub message: Message,
    pub conversation_id: String,
}

#[derive(Debug, Default, Clone)]
pub enum NoticeType {
    #[default]
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct NoticeMessage {
    message: String,
    message_type: NoticeType,
    duration: Option<time::Duration>,
}

impl NoticeMessage {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            message_type: NoticeType::Info,
            duration: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            message_type: NoticeType::Warning,
            duration: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            message_type: NoticeType::Error,
            duration: None,
        }
    }

    pub fn new(message: impl Into<String>) -> Self {
        Self::info(message)
    }

    pub fn with_type(mut self, message_type: NoticeType) -> Self {
        self.message_type = message_type;
        self
    }

    pub fn with_duration(mut self, duration: time::Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn message_type(&self) -> &NoticeType {
        &self.message_type
    }

    pub fn duration(&self) -> Option<time::Duration> {
        self.duration
    }
}

impl NoticeType {
    pub fn border_color(&self) -> Color {
        match self {
            NoticeType::Info => Color::Rgb(30, 136, 229),
            NoticeType::Warning => Color::Rgb(251, 140, 0),
            NoticeType::Error => Color::Rgb(211, 47, 47),
        }
    }

    pub fn text_color(&self) -> Color {
        match self {
            NoticeType::Info => Color::Rgb(144, 202, 249),
            NoticeType::Warning => Color::Rgb(255, 213, 79),
            NoticeType::Error => Color::Rgb(255, 138, 128),
        }
    }
}
