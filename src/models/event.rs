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
macro_rules! info_event {
    ($($arg:tt)*) => {
        Event::Notice($crate::info_notice!($($arg)*))
    }
}

#[macro_export]
macro_rules! warn_event {
    ($($arg:tt)*) => {
        Event::Notice($crate::warn_notice!($($arg)*))
    }
}

#[macro_export]
macro_rules! error_event {
    ($($arg:tt)*) => {
        Event::Notice($crate::error_notice!($($arg)*))
    }
}

#[async_trait::async_trait]
pub trait EventTx {
    async fn send(&self, event: Event) -> Result<(), mpsc::error::SendError<Event>>;
}

impl Event {
    pub fn is_keyboard_event(&self) -> bool {
        matches!(
            self,
            Event::KeyboardCharInput(_)
                | Event::KeyboardEsc
                | Event::KeyboardEnter
                | Event::KeyboardAltEnter
                | Event::KeyboardCtrlC
                | Event::KeyboardCtrlR
                | Event::KeyboardCtrlN
                | Event::KeyboardCtrlE
                | Event::KeyboardCtrlL
                | Event::KeyboardCtrlH
                | Event::KeyboardF1
                | Event::Quit
                | Event::UiScrollUp
                | Event::UiScrollDown
                | Event::UiScrollPageUp
                | Event::UiScrollPageDown
                | Event::KeyboardPaste(_)
        )
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
