pub mod backend;
pub mod message;

use tui_textarea::Input;

pub use crate::backend::*;
pub use crate::message::Message;

pub enum Event {
    BackendMessage(crate::Message),
    BackendPromptResponse(BackendResponse),
    KeyboardCharInput(Input),
    KeyboardEnter,
    KeyboardAltEnter,
    KeyboardCtrlQ,
    KeyboardCtrlC,
    KeyboardCtrlR,
    KeyboardCtrlH,
    KeyboardPaste(String),
    UiTick,
    UiScrollUp,
    UiScrollDown,
    UiScrollPageUp,
    UiScrollPageDown,
}

pub enum Action {
    BackendAbort,
    BackendRequest(BackendPrompt),
    CopyMessages(Vec<Message>),
}
