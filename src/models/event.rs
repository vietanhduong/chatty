use std::sync::Arc;

use tokio::sync::mpsc;
use tui_textarea::Input;

use super::Conversation;

#[derive(Debug)]
pub enum Event {
    Notice(crate::models::NoticeMessage),

    BackendAbort,
    BackendMessage(crate::models::Message),
    BackendPromptResponse(crate::models::BackendResponse),

    SetConversation(Option<Conversation>),
    ConversationDeleted(String),
    ConversationUpdated(Conversation),

    KeyboardCharInput(Input),
    KeyboardEsc,
    KeyboardEnter,
    KeyboardAltEnter,
    KeyboardCtrlC,
    KeyboardCtrlR,
    KeyboardCtrlN,
    KeyboardCtrlE,
    KeyboardCtrlL,
    KeyboardCtrlH,
    KeyboardF1,
    KeyboardPaste(String),

    Quit,

    UiTick,
    UiScrollUp,
    UiScrollDown,
    UiScrollPageUp,
    UiScrollPageDown,
}

#[macro_export]
macro_rules! notice_info {
    ($msg:expr) => {
        Event::Notice($crate::models::NoticeMessage::info($msg))
    };
    ($msg:expr, $duration:expr) => {
        Event::Notice($crate::models::NoticeMessage::info($msg).with_duration($duration))
    };
}

#[macro_export]
macro_rules! notice_warning {
    ($msg:expr) => {
        Event::Notice($crate::models::NoticeMessage::warning($msg))
    };
    ($msg:expr, $duration:expr) => {
        Event::Notice($crate::models::NoticeMessage::warning($msg).with_duration($duration))
    };
}

#[macro_export]
macro_rules! notice_error {
    ($msg:expr) => {
        Event::Notice($crate::models::NoticeMessage::error($msg))
    };
    ($msg:expr, $duration:expr) => {
        Event::Notice($crate::models::NoticeMessage::error($msg).with_duration($duration))
    };
}

#[async_trait::async_trait]
pub trait EventTx {
    async fn send(&self, event: Event) -> Result<(), mpsc::error::SendError<Event>>;
}

impl Event {
    pub fn is_keyboard_event(&self) -> bool {
        match self {
            Event::KeyboardCharInput(_) => true,
            Event::KeyboardEsc => true,
            Event::KeyboardEnter => true,
            Event::KeyboardAltEnter => true,
            Event::Quit => true,
            Event::KeyboardCtrlC => true,
            Event::KeyboardCtrlR => true,
            Event::KeyboardCtrlN => true,
            Event::KeyboardCtrlE => true,
            Event::KeyboardCtrlL => true,
            Event::KeyboardCtrlH => true,
            Event::KeyboardF1 => true,
            Event::UiScrollUp => true,
            Event::UiScrollDown => true,
            Event::UiScrollPageUp => true,
            Event::UiScrollPageDown => true,
            _ => false,
        }
    }
}

#[async_trait::async_trait]
impl EventTx for mpsc::Sender<Event> {
    async fn send(&self, event: Event) -> Result<(), mpsc::error::SendError<Event>> {
        self.send(event).await
    }
}

#[async_trait::async_trait]
impl EventTx for mpsc::UnboundedSender<Event> {
    async fn send(&self, event: Event) -> Result<(), mpsc::error::SendError<Event>> {
        self.send(event)
    }
}

pub type ArcEventTx = Arc<dyn EventTx + Send + Sync>;
