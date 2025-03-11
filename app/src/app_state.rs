use openai_models::{BackendResponse, Message, message::Issuer};
use ratatui::layout::Rect;
use syntect::highlighting::Theme;

use crate::{ui::BubbleList, ui::Scroll};

pub struct AppState<'a> {
    pub(crate) bubble_list: BubbleList<'a>,
    pub(crate) last_known_height: usize,
    pub(crate) last_known_width: usize,
    pub(crate) messages: Vec<Message>,
    pub(crate) scroll: Scroll,
    pub(crate) waiting_for_backend: bool,
}

impl<'a> AppState<'_> {
    pub fn new(theme: &'a Theme) -> AppState<'a> {
        let mut app_state = AppState {
            bubble_list: BubbleList::new(theme),
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

    pub fn last_message_of(&self, issuer: Option<Issuer>) -> Option<&Message> {
        if let Some(msg) = self.messages.last() {
            if issuer.is_none() {
                return Some(msg);
            }

            let value;
            let is_system = match msg.issuer() {
                Issuer::System(sys) => {
                    value = sys.to_string();
                    true
                }
                Issuer::User(val) => {
                    value = val.to_string();
                    false
                }
            };

            if is_system == msg.is_system() {
                if value.is_empty() {
                    return Some(msg);
                }
                return if msg.issuer_str() == value {
                    Some(msg)
                } else {
                    None
                };
            }
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

    pub fn sync_state(&mut self) {
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
