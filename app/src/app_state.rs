use openai_models::{BackendResponse, Conversation, Message, message::Issuer};
use ratatui::layout::Rect;
use syntect::highlighting::Theme;

use crate::{ui::BubbleList, ui::Scroll};

pub(crate) enum MessageAction<'a> {
    UpdateMessage(&'a Message),
    InsertMessage(&'a Message),
}

pub struct AppState<'a> {
    theme: &'a Theme,
    pub(crate) bubble_list: BubbleList<'a>,
    pub(crate) last_known_height: usize,
    pub(crate) last_known_width: usize,
    pub(crate) conversation: Conversation,
    pub(crate) scroll: Scroll,
    pub(crate) waiting_for_backend: bool,
}

impl<'a> AppState<'_> {
    pub fn new(theme: &'a Theme) -> AppState<'a> {
        let mut app_state = AppState {
            theme,
            bubble_list: BubbleList::new(theme),
            last_known_height: 0,
            last_known_width: 0,
            conversation: Conversation::default(),
            scroll: Scroll::default(),
            waiting_for_backend: false,
        };

        app_state.conversation.add_message(Message::new_system(
            "system",
            "Hello! How can I help you? 😊",
        ));

        app_state
    }

    pub fn set_conversation(&mut self, conversation: Conversation) {
        self.conversation = conversation;
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
        self.conversation.add_message(message);
        self.sync_state();
        self.scroll.last();
    }

    pub fn last_message_of(&self, issuer: Option<Issuer>) -> Option<&Message> {
        if let Some(msg) = self.conversation.last_message() {
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

    pub(crate) fn handle_backend_response(&mut self, msg: BackendResponse) -> MessageAction {
        let last_message = self.conversation.last_mut_message().unwrap();
        let insert = !last_message.is_system();
        if last_message.is_system() {
            last_message.append(&msg.text);
        } else {
            self.conversation
                .add_message(Message::new_system(msg.model.as_str(), &msg.text).with_id(msg.id));
        };

        self.sync_state();

        if msg.done {
            if msg.init_conversation {
                // The init convesrsation message will contain the title of
                // the conversation at the beginning of the text and starts with #

                // Get the first line of the last message
                let message = self.conversation.last_message().unwrap();
                let first_line = message.text().lines().next().unwrap_or("");
                // Check if the first line starts with #
                if first_line.starts_with('#') {
                    // Remove the # and any leading spaces
                    let title = first_line.trim_start_matches('#').trim();
                    // Set the title of the conversation
                    self.conversation.set_title(title.to_string());
                }
            }

            self.conversation.set_updated_at(chrono::Utc::now());
            self.waiting_for_backend = false;
            if let Some(ctx) = msg.context {
                self.conversation.set_context(ctx);
            }

            if self.conversation.context().unwrap_or_default().is_empty() {
                self.add_message(Message::new_system(
                    "system",
                    "No context available for this code.",
                ));
            }
        }

        let last_message = self.conversation.last_message().unwrap();

        if insert {
            MessageAction::InsertMessage(&last_message)
        } else {
            MessageAction::UpdateMessage(&last_message)
        }
    }

    pub fn sync_state(&mut self) {
        self.bubble_list
            .set_messages(self.conversation.messages(), self.last_known_width);
        let scrollbar_at_bottom = self.scroll.is_position_at_last();
        self.scroll
            .set_state(self.bubble_list.len(), self.last_known_height);
        if self.waiting_for_backend && scrollbar_at_bottom {
            self.scroll.last();
        }
    }
}
