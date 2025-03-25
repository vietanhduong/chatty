use std::{cell::RefCell, rc::Rc};

use openai_models::{BackendResponse, Conversation, Message};
use ratatui::layout::Rect;
use syntect::highlighting::Theme;

use crate::{ui::BubbleList, ui::Scroll};

pub struct AppState<'a> {
    theme: &'a Theme,
    pub(crate) bubble_list: BubbleList<'a>,
    pub(crate) last_known_height: usize,
    pub(crate) last_known_width: usize,
    pub(crate) conversation: Rc<RefCell<Conversation>>,
    pub(crate) scroll: Scroll,
    pub(crate) waiting_for_backend: bool,
}

impl<'a> AppState<'a> {
    pub fn new(conversation: Rc<RefCell<Conversation>>, theme: &'a Theme) -> AppState<'a> {
        AppState {
            theme,
            bubble_list: BubbleList::new(theme),
            last_known_height: 0,
            last_known_width: 0,
            conversation,
            scroll: Scroll::default(),
            waiting_for_backend: false,
        }
    }

    pub fn set_conversation(&mut self, conversation: Rc<RefCell<Conversation>>) {
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
        {
            let mut conversation = self.conversation.borrow_mut();
            conversation.add_message(message);
        }
        self.sync_state();
        self.scroll.last();
    }

    pub(crate) fn last_message(&self) -> Option<Message> {
        self.conversation.borrow().last_message().cloned()
    }

    pub(crate) fn handle_backend_response(&mut self, resp: &BackendResponse) {
        {
            let mut conversation = self.conversation.borrow_mut();
            let last_message = conversation.last_message().unwrap();
            if !last_message.is_system() {
                conversation.add_message(Message::new_system(&resp.model, "").with_id(&resp.id));
            }
            conversation.last_mut_message().unwrap().append(&resp.text);
        }

        if resp.done {
            if resp.init_conversation {
                // The init convesrsation message will contain the title of
                // the conversation at the beginning of the text and starts with #

                // Get the first line of the last message
                let message = self.last_message().unwrap();
                let first_line = message.text().lines().next().unwrap_or("");
                // Check if the first line starts with #
                if first_line.starts_with('#') {
                    // Remove the # and any leading spaces
                    let title = first_line.trim_start_matches('#').trim();
                    if !title.is_empty() {
                        // Set the title of the conversation
                        self.conversation.borrow_mut().set_title(title.to_string());
                    }
                }
            }

            self.conversation
                .borrow_mut()
                .set_updated_at(chrono::Utc::now());
            self.waiting_for_backend = false;
        }
        self.sync_state();
    }

    pub fn sync_state(&mut self) {
        self.bubble_list
            .set_messages(self.conversation.borrow().messages(), self.last_known_width);
        let scrollbar_at_bottom = self.scroll.is_position_at_last();
        self.scroll
            .set_state(self.bubble_list.len(), self.last_known_height);
        if self.waiting_for_backend && scrollbar_at_bottom {
            self.scroll.last();
        }
    }
}
