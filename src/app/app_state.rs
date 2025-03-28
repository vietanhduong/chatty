use crate::models::{BackendResponse, Conversation, Message};
use ratatui::layout::Rect;
use syntect::highlighting::Theme;

use crate::{app::ui::BubbleList, app::ui::Scroll};

pub(crate) struct AppState<'a> {
    theme: &'a Theme,
    pub bubble_list: BubbleList<'a>,
    pub last_known_height: usize,
    pub last_known_width: usize,
    pub scroll: Scroll,

    pub current_convo: Conversation,
    pub waiting_for_backend: bool,
}

impl<'a> AppState<'a> {
    pub fn new(theme: &'a Theme) -> AppState<'a> {
        AppState {
            theme,
            bubble_list: BubbleList::new(theme),
            last_known_height: 0,
            last_known_width: 0,
            current_convo: Conversation::new_hello(),
            scroll: Scroll::default(),
            waiting_for_backend: false,
        }
    }

    pub fn set_conversation(&mut self, convo: Conversation) {
        self.current_convo = convo;
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
        self.current_convo.append_message(message);
        self.sync_state();
        self.scroll.last();
    }

    pub fn handle_backend_response(&mut self, resp: &BackendResponse) {
        if self.current_convo.len() == 0
            || matches!(self.current_convo.messages().last(), Some(last) if !last.is_system())
        {
            self.current_convo
                .append_message(Message::new_system(&resp.model, "").with_id(&resp.id));
        }

        {
            let last_message = self.current_convo.last_mut_message().unwrap();
            last_message.append(&resp.text);
        }

        if resp.done {
            if resp.init_conversation {
                // The init convesrsation message will contain the title of
                // the conversation at the beginning of the text and starts with #

                // Get the first line of the last message
                let first_line = self
                    .current_convo
                    .messages()
                    .last()
                    .unwrap()
                    .text()
                    .lines()
                    .next()
                    .unwrap_or("");
                // Check if the first line starts with #
                if first_line.starts_with('#') {
                    // Remove the # and any leading spaces
                    let title = first_line.trim_start_matches('#').trim();
                    if !title.is_empty() {
                        // Set the title of the conversation
                        self.current_convo.set_title(title.to_string());
                    }
                }
            }
            let updated_at = self.current_convo.last_mut_message().unwrap().created_at();
            self.current_convo.set_updated_at(updated_at);
            self.waiting_for_backend = false;
        }
        self.sync_state();
    }

    pub fn sync_state(&mut self) {
        self.bubble_list
            .set_messages(self.current_convo.messages(), self.last_known_width);
        let scrollbar_at_bottom = self.scroll.is_position_at_last();
        self.scroll
            .set_state(self.bubble_list.len(), self.last_known_height);
        if self.waiting_for_backend && scrollbar_at_bottom {
            self.scroll.last();
        }
    }
}
