use openai_models::{BackendResponse, Message, message::Issuer};
use ratatui::layout::Rect;

use crate::{ui::BubbleList, ui::Scroll};

pub struct AppState<'a> {
    pub bubble_list: BubbleList<'a>,
    pub last_known_height: usize,
    pub last_known_width: usize,
    pub messages: Vec<Message>,
    pub scroll: Scroll,
    pub waiting_for_backend: bool,
}

impl<'a> AppState<'_> {
    pub fn new() -> Self {
        let mut app_state = AppState {
            bubble_list: BubbleList::new(),
            last_known_height: 0,
            last_known_width: 0,
            messages: Vec::new(),
            scroll: Scroll::default(),
            waiting_for_backend: false,
        };

        app_state
            .messages
            .push(Message::new_system("system", "Hello! How can I help you?"));

        app_state
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.last_known_height = rect.height.into();
        self.last_known_width = rect.width.into();
        self.sync_state();
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.sync_state();
        self.scroll.last();
    }

    pub fn last_message(&self, is_system: Option<bool>) -> Option<&Message> {
        if let Some(msg) = self.messages.last() {
            if is_system.is_none() || is_system.unwrap() == msg.is_system() {
                return Some(msg);
            }
        }
        None
    }

    pub fn pop_last_message(&mut self, is_system: Option<bool>) -> Option<Message> {
        if self.last_message(is_system).is_some() {
            let ret = self.messages.pop();
            if let Some(ref msg) = ret {
                self.bubble_list.remove_message(msg.id());
                self.sync_state();
            }
            return ret;
        }
        None
    }

    pub fn handle_backend_response(&mut self, msg: BackendResponse) {
        let last_message = self.messages.last_mut().unwrap();
        if last_message.is_system() {
            last_message.append(&msg.text);
        } else {
            self.messages
                .push(Message::new_system(msg.model.as_str(), &msg.text).with_id(msg.id));
        }

        self.sync_state();

        if msg.done {
            self.waiting_for_backend = false;
        }
    }

    pub fn chat_context(&self) -> Vec<&Message> {
        // Filter all message except system ("system") messages
        self.messages
            .iter()
            .filter(|msg| match msg.issuer() {
                Issuer::System(sys) => sys != "system",
                _ => true,
            })
            .collect::<Vec<_>>()
    }

    fn sync_state(&mut self) {
        self.bubble_list
            .set_messages(&self.messages, self.last_known_width);
        let scrollbar_at_bottom = self.scroll.is_position_at_last();
        self.scroll
            .set_state(self.bubble_list.len(), self.last_known_height);
        if self.waiting_for_backend && scrollbar_at_bottom {
            self.scroll.last();
        }
    }
}
