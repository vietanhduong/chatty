pub mod backend;
pub mod configuration;
pub mod message;

use tui_textarea::Input;

pub use crate::backend::*;
pub use crate::message::Message;

pub use crate::configuration as config;

#[derive(Debug)]
pub enum Event {
    AbortRequest,
    BackendMessage(crate::Message),
    BackendPromptResponse(BackendResponse),
    KeyboardCharInput(Input),
    KeyboardEsc,
    KeyboardEnter,
    KeyboardAltEnter,
    KeyboardCtrlQ,
    KeyboardCtrlC,
    KeyboardCtrlR,
    KeyboardCtrlH,
    KeyboardF1,
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
