use openai_models::{BackendResponse, Converstation, Message, message::Issuer};
use ratatui::layout::Rect;
use syntect::highlighting::Theme;

use crate::{ui::BubbleList, ui::Scroll};

pub struct AppState<'a> {
    theme: &'a Theme,
    pub(crate) bubble_list: BubbleList<'a>,
    pub(crate) last_known_height: usize,
    pub(crate) last_known_width: usize,
    pub(crate) converstation: Converstation,
    pub(crate) scroll: Scroll,
    pub(crate) waiting_for_backend: bool,
    pub(crate) context: String,
}

impl<'a> AppState<'_> {
    pub fn new(theme: &'a Theme) -> AppState<'a> {
        let mut app_state = AppState {
            theme,
            bubble_list: BubbleList::new(theme),
            last_known_height: 0,
            last_known_width: 0,
            converstation: Converstation::default(),
            scroll: Scroll::default(),
            waiting_for_backend: false,
            context: String::new(),
        };

        app_state.converstation.add_message(Message::new_system(
            "system",
            "Hello! How can I help you? 😊",
        ));

        app_state
    }

    pub fn set_converstation(&mut self, converstation: Converstation) {
        self.converstation = converstation;
        self.bubble_list = BubbleList::new(self.theme);
        self.sync_state();
        // Move the scroll to the last message
        self.scroll.last();
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.last_known_height = rect.height.into();
        self.last_known_width = rect.width.into();
        self.sync_state();
    }

    pub fn add_message(&mut self, message: Message) {
        self.converstation.add_message(message);
        self.sync_state();
        self.scroll.last();
    }

    pub fn last_message_of(&self, issuer: Option<Issuer>) -> Option<&Message> {
        if let Some(msg) = self.converstation.last_message() {
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
        let last_message = self.converstation.last_mut_message().unwrap();
        if last_message.is_system() {
            last_message.append(&msg.text);
        } else {
            self.converstation
                .add_message(Message::new_system(msg.model.as_str(), &msg.text).with_id(msg.id));
        }

        self.sync_state();

        if msg.done {
            if msg.init_conversation {
                // The init convesrsation message will contain the title of
                // the conversation at the beginning of the text and starts with #

                // Get the first line of the last message
                let message = self.converstation.last_message().unwrap();
                let first_line = message.text().lines().next().unwrap_or("");
                // Check if the first line starts with #
                if first_line.starts_with('#') {
                    // Remove the # and any leading spaces
                    let title = first_line.trim_start_matches('#').trim();
                    // Set the title of the conversation
                    self.converstation.set_title(title.to_string());
                }
            }

            self.waiting_for_backend = false;
            if let Some(ctx) = msg.context {
                self.context = ctx;
            }

            if self.context.is_empty() {
                self.add_message(Message::new_system(
                    "system",
                    "No context available for this code.",
                ));
            }
        }
    }

    pub fn sync_state(&mut self) {
        self.bubble_list
            .set_messages(self.converstation.messages(), self.last_known_width);
        let scrollbar_at_bottom = self.scroll.is_position_at_last();
        self.scroll
            .set_state(self.bubble_list.len(), self.last_known_height);
        if self.waiting_for_backend && scrollbar_at_bottom {
            self.scroll.last();
        }
    }
}
