use crossterm::event::KeyEvent;

use super::NoticeMessage;

#[derive(Debug)]
pub enum TaskEvent {
    AddTask(Task),
    AddNotice(NoticeMessage),
    CompleteTask {
        id: String,
        suffix_message: Option<String>,
        success: bool,
    },
    Complete,

    CrosstermKey(KeyEvent),
    UiTick,
}

#[derive(Debug)]
pub struct Task {
    pub id: String,
    pub message: String,
}
