use crate::models::{Action, Event, Message};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Text},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Widget,
    },
};
use ratatui_macros::span;
use syntect::highlighting::Theme;
use tokio::sync::mpsc;
use tui_textarea::Key;
use unicode_width::UnicodeWidthStr;

use super::{Dim, utils};

pub struct EditScreen<'a> {
    action_tx: mpsc::UnboundedSender<Action>,

    theme: &'a Theme,

    showing: bool,
    messages: Vec<SelectedMessage>,
    list_state: ListState,
}

impl<'a> EditScreen<'_> {
    pub fn new(theme: &'a Theme, action_tx: mpsc::UnboundedSender<Action>) -> EditScreen<'a> {
        EditScreen {
            action_tx,

            showing: false,
            messages: vec![],
            list_state: ListState::default(),
            theme,
        }
    }

    pub fn set_messages(&mut self, messages: &[Message]) {
        self.messages = messages
            .iter()
            .map(|message| SelectedMessage {
                msg: message.clone(),
                selected: false,
            })
            .collect();
        // Sort the messages by the created time descending
        self.messages
            .sort_by(|a, b| b.msg.created_at().cmp(&a.msg.created_at()));
        if !self.messages.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn toggle_showing(&mut self) {
        self.showing = !self.showing;
    }

    fn toggle_selected(&mut self) {
        if let Some(i) = self.list_state.selected() {
            self.messages[i].selected = !self.messages[i].selected;
        }
    }

    fn next_row(&mut self) {
        self.list_state.select_next();
    }

    fn prev_row(&mut self) {
        self.list_state.select_previous();
    }

    fn pageup(&mut self) {
        [..10]
            .iter()
            .for_each(|_| self.list_state.select_previous());
    }

    fn pagedown(&mut self) {
        [..10].iter().for_each(|_| self.list_state.select_next());
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.showing {
            return;
        }

        f.dim_bg();

        let instructions = vec![
            span!(" "),
            span!("q").green().bold(),
            span!(" to close, ").white(),
            span!("Space").green().bold(),
            span!(" to select, ").white(),
            span!("y").green().bold(),
            span!(" to copy selected, ").white(),
            span!("c").green().bold(),
            span!(" to quick copy ").white(),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightBlue))
            .padding(Padding::symmetric(1, 0))
            .title(Line::from(" Edit Mode ").bold())
            .title_alignment(Alignment::Center)
            .title_bottom(Line::from(instructions))
            .style(Style::default());

        f.render_widget(Clear, area);
        let inner = block.inner(area);
        f.render_widget(block, area);

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(inner);
        self.render_messages_panel(f, layout[0]);
        self.render_preview_panel(f, layout[1]);
    }

    fn render_messages_panel(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::symmetric(1, 0))
            .title(" Messages (Newer First) ")
            .title_alignment(Alignment::Left)
            .style(Style::default());
        let inner = block.inner(area);
        let max_width = inner.width as usize;
        let messages = build_list_items(&self.messages, max_width - 2);
        let list = List::new(messages)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn render_preview_panel(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::symmetric(1, 0))
            .title(" Preview ")
            .title_alignment(Alignment::Left)
            .style(Style::default());

        let i = match self.list_state.selected() {
            Some(i) => i,
            None => {
                frame.render_widget(block, area);
                return;
            }
        };

        if i >= self.messages.len() || self.messages.is_empty() {
            frame.render_widget(block, area);
            return;
        }

        let message = &self.messages[i].msg;
        let lines = utils::build_message_lines(
            message.text(),
            area.width as usize - 5,
            self.theme,
            Line::from,
        );

        let text = Text::from(lines);
        let paragraph = Paragraph::new(text).block(block).alignment(Alignment::Left);
        paragraph.render(area, frame.buffer_mut());
    }

    pub async fn handle_key_event(&mut self, event: &Event) -> bool {
        match event {
            Event::KeyboardCtrlE => {
                self.showing = !self.showing;
                return false;
            }
            Event::Quit => {
                self.showing = false;
                return true;
            }

            Event::KeyboardCharInput(input) => match input.key {
                Key::Char('c') => {
                    if let Some(i) = self.list_state.selected() {
                        let message = self.messages[i].msg.clone();
                        let _ = self.action_tx.send(Action::CopyMessages(vec![message]));
                    }
                }
                Key::Char('y') => {
                    let mut selected_messages: Vec<Message> = self
                        .messages
                        .iter()
                        .filter(|msg| msg.selected)
                        .map(|msg| msg.msg.clone())
                        .collect();

                    selected_messages.sort_by_key(|msg| msg.created_at());

                    if !selected_messages.is_empty() {
                        let _ = self.action_tx.send(Action::CopyMessages(selected_messages));
                    }
                }
                Key::Char('j') => self.next_row(),
                Key::Char('k') => self.prev_row(),
                Key::Char('g') => self.list_state.select(Some(0)),
                Key::Char('G') => {
                    if self.list_state.selected().is_some() {
                        self.list_state.select(Some(self.messages.len() - 1));
                    }
                }
                Key::Char(' ') => self.toggle_selected(),
                Key::Char('q') => {
                    self.showing = false;
                    return false;
                }
                _ => {}
            },

            Event::UiScrollUp => self.prev_row(),
            Event::UiScrollDown => self.next_row(),
            Event::UiScrollPageUp => self.pageup(),
            Event::UiScrollPageDown => self.pagedown(),

            _ => {}
        }
        false
    }
}

fn build_list_items<'a>(messages: &[SelectedMessage], max_width: usize) -> Vec<ListItem<'a>> {
    messages
        .iter()
        .map(|item| {
            let mut spans = vec![];
            if item.selected {
                spans.push(span!(Style::default().fg(Color::Red); "[*]"));
            } else {
                spans.push(span!(Style::default(); "[ ]"));
            }
            spans.push(span!(Style::default(); " "));

            let fg_color = if item.msg.is_system() {
                Color::LightCyan
            } else {
                Color::LightMagenta
            };

            let mut content = format!(
                "{}: {}",
                if item.msg.is_system() { "S" } else { "U" },
                item.msg.text()
            );
            // If the content is too long, we will truncate it
            // and add ellipsis
            if content.width() > max_width {
                content = content.chars().take(max_width).collect::<String>();
            }

            spans.push(span!(Style::default(); content));
            let text = Text::from(Line::from(spans));
            ListItem::new(text).fg(fg_color)
        })
        .collect()
}

struct SelectedMessage {
    msg: Message,
    selected: bool,
}
