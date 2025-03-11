use openai_models::{Event, Message};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding},
};
use syntect::highlighting::Theme;
use tui_textarea::Key;
use unicode_width::UnicodeWidthStr;

use super::helpers;

pub struct EditScreen<'a> {
    showing: bool,
    messages: Vec<SelectedMessage>,
    list_state: ListState,
    theme: &'a Theme,
}

impl<'a> EditScreen<'_> {
    pub fn new(theme: &'a Theme) -> EditScreen<'a> {
        EditScreen {
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

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightBlue))
            .padding(Padding::symmetric(1, 0))
            .title(" Edit Mode ")
            .title_alignment(Alignment::Center)
            .title_bottom(" <Esc> to close ")
            .style(Style::default());
        let area = helpers::popup_area(area, 70, 90);
        frame.render_widget(Clear, area);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(inner);
        self.render_messages_panel(frame, layout[0]);
        self.render_preview_panel(frame, layout[1]);
    }

    fn render_messages_panel(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::symmetric(1, 0))
            .title(" Messages ")
            .title_alignment(Alignment::Left)
            .style(Style::default());
        // frame.render_widget(block, area);
        let max_width = area.width as usize;
        let messages = build_list_items(&self.messages, max_width);
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
        let lines = helpers::build_message_lines(
            message.text(),
            area.width as usize - 5,
            self.theme,
            |spans| Line::from(spans),
        );

        let text = Text::from(lines);
        let list = List::new(text)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_widget(list, area);
    }

    pub fn handle_key_event(&mut self, event: Event) -> bool {
        match event {
            Event::KeyboardEsc => {
                self.showing = false;
                return false;
            }
            Event::KeyboardCtrlE => {
                self.showing = !self.showing;
                return false;
            }
            Event::KeyboardCtrlQ => {
                self.showing = false;
                return true;
            }

            Event::KeyboardCharInput(input) => match input.key {
                Key::Char('j') => self.next_row(),
                Key::Char('k') => self.prev_row(),
                Key::Char('g') => self.list_state.select(Some(0)),
                Key::Char('G') => {
                    if let Some(_) = self.list_state.selected() {
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
            let mut content = format!(
                "[{}] {}: {}",
                if item.selected { "x" } else { " " },
                if item.msg.is_system() { "S" } else { "U" },
                item.msg.text()
            );
            // If the content is too long, we will truncate it
            // and add ellipsis
            if content.width() > max_width - 5 {
                let mut truncated = content.chars().take(max_width - 5).collect::<String>();
                truncated.push_str("...");
                content = truncated;
            }

            let text = Text::from(Line::from(content));
            ListItem::new(text)
        })
        .collect()
}

struct SelectedMessage {
    msg: Message,
    selected: bool,
}
