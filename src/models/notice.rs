use ratatui::style::Color;

#[macro_export]
macro_rules! info_notice {
    ($msg:expr) => {
        $crate::models::NoticeMessage::info($msg)
    };
    ($msg:expr, $duration:expr) => {
        $crate::models::NoticeMessage::info($msg).with_duration($duration)
    };
}

#[macro_export]
macro_rules! warn_notice {
    ($msg:expr) => {
        $crate::models::NoticeMessage::warning($msg)
    };
    ($msg:expr, $duration:expr) => {
        $crate::models::NoticeMessage::warning($msg).with_duration($duration)
    };
}

#[macro_export]
macro_rules! error_notice {
    ($msg:expr) => {
        $crate::models::NoticeMessage::error($msg)
    };
    ($msg:expr, $duration:expr) => {
        $crate::models::NoticeMessage::error($msg).with_duration($duration)
    };
}

#[derive(Debug, Default, Clone)]
pub enum NoticeKind {
    #[default]
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct NoticeMessage {
    message: String,
    kind: NoticeKind,
    duration: Option<std::time::Duration>,
}

impl NoticeMessage {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: NoticeKind::Info,
            duration: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: NoticeKind::Warning,
            duration: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: NoticeKind::Error,
            duration: None,
        }
    }

    pub fn new(message: impl Into<String>) -> Self {
        Self::info(message)
    }

    pub fn with_kind(mut self, kind: NoticeKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_duration(mut self, duration: std::time::Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn kind(&self) -> &NoticeKind {
        &self.kind
    }

    pub fn duration(&self) -> Option<std::time::Duration> {
        self.duration
    }
}

impl NoticeKind {
    pub fn border_color(&self) -> Color {
        match self {
            NoticeKind::Info => Color::Rgb(30, 136, 229),
            NoticeKind::Warning => Color::Rgb(251, 140, 0),
            NoticeKind::Error => Color::Rgb(211, 47, 47),
        }
    }

    pub fn text_color(&self) -> Color {
        match self {
            NoticeKind::Info => Color::Rgb(144, 202, 249),
            NoticeKind::Warning => Color::Rgb(255, 213, 79),
            NoticeKind::Error => Color::Rgb(255, 138, 128),
        }
    }
}
