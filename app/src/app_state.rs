use openai_models::{BackendResponse, Message};
use ratatui::layout::Rect;

use crate::{ui::BubbleList, ui::CodeBlocks, ui::Scroll};

pub struct AppState<'a> {
    pub backend_context: String,
    pub bubble_list: BubbleList<'a>,
    pub codeblocks: CodeBlocks,
    pub last_known_height: usize,
    pub last_known_width: usize,
    pub messages: Vec<Message>,
    pub scroll: Scroll,
    pub waiting_for_backend: bool,
}

impl<'a> AppState<'_> {
    pub fn new() -> Self {
        let mut app_state = AppState {
            backend_context: String::new(),
            bubble_list: BubbleList::new(),
            codeblocks: CodeBlocks::default(),
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

    pub fn handle_backend_response(&mut self, msg: BackendResponse) {
        let last_message = self.messages.last_mut().unwrap();
        if last_message.is_system() {
            last_message.append(&msg.text);
        } else {
            self.messages
                .push(Message::new_system(msg.model.as_str(), &msg.text));
        }

        self.sync_state();

        if msg.done {
            self.waiting_for_backend = false;
            if let Some(ctx) = msg.context {
                self.backend_context = ctx;
            }
            if self.backend_context.is_empty() {
                self.add_message(Message::new_system(
                    "system",
                    "No context available. Please provide a context.",
                ));
                self.sync_state();
            }

            self.codeblocks.replace_from_messages(&self.messages);
        }
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
