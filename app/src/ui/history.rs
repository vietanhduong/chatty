use eyre::Result;
use openai_models::{Conversation, Event};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding},
};
use ratatui_macros::span;
use std::{cell::RefCell, rc::Rc};
use tui_textarea::Key;

use super::helpers;

const NO_CONVERSATIONS: &str = "No conversations found";

#[derive(Default)]
pub struct HistoryScreen<'a> {
    showing: bool,
    conversations: Vec<Rc<RefCell<Conversation>>>,
    list_items: Vec<ListItem<'a>>,

    current_conversation: Option<String>,
    list_state: ListState,
}

impl<'a> HistoryScreen<'a> {
    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn with_conversations(
        mut self,
        conversations: Vec<Rc<RefCell<Conversation>>>,
    ) -> HistoryScreen<'a> {
        self.conversations = conversations;
        // sort the conversations by last updated time descending
        self.conversations
            .sort_by(|a, b| b.borrow().updated_at().cmp(&a.borrow().updated_at()));
        self.conversations
            .dedup_by(|a, b| a.borrow().id() == b.borrow().id());
        self
    }

    pub fn with_current_conversation(
        mut self,
        current_conversation: impl Into<String>,
    ) -> HistoryScreen<'a> {
        self.current_conversation = Some(current_conversation.into());
        self
    }

    pub fn toggle_showing(&mut self) {
        self.showing = !self.showing;
    }

    pub fn add_conversation(&mut self, conversation: Rc<RefCell<Conversation>>) {
        // If the conversation already exists, just update it
        // otherwise, add the conversation at the top of the list
        let pos = self
            .conversations
            .iter()
            .position(|c| c.borrow().id() == conversation.borrow().id())
            .unwrap_or_default();

        if pos != 0 {
            log::debug!(
                "Conversation already exists, updating it: {:?}",
                conversation
            );
            // remove the conversation from the list
            self.conversations.remove(pos);
        }

        self.current_conversation = Some(conversation.borrow().id().to_string());
        self.conversations.insert(0, conversation);
        // sort the conversations by last updated time descending
        self.conversations
            .sort_by(|a, b| b.borrow().updated_at().cmp(&a.borrow().updated_at()));
    }

    pub fn get_conversations(&self) -> &[Rc<RefCell<Conversation>>] {
        &self.conversations
    }

    pub fn current_conversation(&self) -> Option<String> {
        self.current_conversation.clone()
    }

    pub fn set_current_conversation(&mut self, conversation: impl Into<String>) {
        self.current_conversation = Some(conversation.into());
    }

    fn next_row(&mut self) {
        if self.conversations.is_empty() {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => (i + 1).min(self.list_items.len() - 1),
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn prev_row(&mut self) {
        if self.conversations.is_empty() {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => (i as isize - 1).max(0) as usize,
            None => 0,
        };

        self.list_state.select(Some(i));
    }

    fn pageup(&mut self) {
        for _ in 0..10 {
            self.prev_row();
        }
    }

    fn pagedown(&mut self) {
        for _ in 0..10 {
            self.next_row();
        }
    }

    fn build_list_items(&mut self, max_width: usize) {
        self.list_items.clear();

        if self.conversations.is_empty() {
            self.list_items.push(ListItem::new(
                Text::from(NO_CONVERSATIONS).alignment(Alignment::Center),
            ));
            self.list_state.select(None);
            return;
        }

        for c in &self.conversations {
            let mut spans = c
                .borrow()
                .title()
                .split(' ')
                .map(|s| Span::raw(s.to_string()))
                .collect::<Vec<_>>();
            if self.current_conversation.as_deref() == Some(c.borrow().id()) {
                spans.push(Span::styled(
                    "[current]",
                    Style::default().fg(Color::LightRed),
                ))
            }

            let lines = helpers::split_to_lines(spans, max_width);
            self.list_items.push(ListItem::new(Text::from(lines)));
        }
    }

    pub async fn handle_key_event(&mut self, event: Event) -> Result<bool> {
        match event {
            Event::KeyboardEsc => {
                self.showing = false;
                return Ok(false);
            }
            Event::KeyboardCtrlH => {
                self.showing = !self.showing;
                return Ok(false);
            }
            Event::KeyboardCtrlQ => {
                self.showing = false;
                return Ok(true);
            }

            Event::KeyboardEnter => {
                if self.list_state.selected().is_none() || self.conversations.is_empty() {
                    return Ok(false);
                }
                if let Some(i) = self.list_state.selected() {
                    let conversation = match self.conversations.get(i) {
                        Some(c) => c,
                        None => {
                            return Ok(false);
                        }
                    };
                    self.current_conversation = Some(conversation.borrow().id().to_string());
                    self.showing = false;
                }
                return Ok(false);
            }

            Event::KeyboardCharInput(input) => match input.key {
                Key::Char('j') => self.next_row(),
                Key::Char('k') => self.prev_row(),
                Key::Char('q') => {
                    self.showing = false;
                    return Ok(false);
                }
                _ => {}
            },

            Event::UiScrollUp => self.prev_row(),
            Event::UiScrollDown => self.next_row(),
            Event::UiScrollPageUp => self.pageup(),
            Event::UiScrollPageDown => self.pagedown(),

            _ => {}
        }
        Ok(false)
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.showing {
            return;
        }

        let instructions: Vec<Span> = vec![
            " ".into(),
            span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "Esc"),
            " to close, ".into(),
            span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "Enter"),
            " to select, ".into(),
            span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "↑/k/↓/j"),
            " to move up/down ".into(),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightBlue))
            .padding(Padding::new(1, 1, 0, 0))
            .title(" Chat History ")
            .title_alignment(Alignment::Center)
            .title_bottom(Line::from(instructions))
            .style(Style::default());

        f.render_widget(Clear, area);
        let inner = block.inner(area);
        self.build_list_items((inner.width - 2) as usize);

        let list = List::new(self.list_items.clone())
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        f.render_stateful_widget(list, inner, &mut self.list_state);
    }

    pub fn append_current_span(&self, lines: &mut Vec<Line>) {
        // get last line and append the [current] span to it
        if let Some(last_line) = lines.last_mut() {
            last_line.spans.push(Span::styled(
                " [current]",
                Style::default().fg(Color::LightRed),
            ));
        }
    }
}
