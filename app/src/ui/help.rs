use once_cell::sync::Lazy;
use openai_models::Event;
use ratatui_macros::span;
use std::{
    cmp::{max, min},
    fmt::Display,
};

use ratatui::{
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, BorderType, Borders, Cell, Clear, Padding, Row, Table, TableState},
};
use tui_textarea::Key;

use super::helpers;

pub const KEY_BINDINGS: Lazy<Vec<KeyBinding>> = Lazy::new(build_key_bindings);
const ROW_HEIGHT: usize = 1;

pub struct HelpScreen<'a> {
    showing: bool,

    state: TableState,

    rows: Vec<Row<'a>>,
    last_known_width: usize,
    last_know_height: usize,
}

impl<'a> HelpScreen<'_> {
    pub fn new() -> HelpScreen<'a> {
        HelpScreen {
            showing: false,
            state: TableState::default().with_selected(0),
            rows: vec![],
            last_known_width: 0,
            last_know_height: 0,
        }
    }

    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn toggle_showing(&mut self) {
        self.showing = !self.showing;
    }

    fn next_row(&mut self) {
        let i = match self.state.selected() {
            Some(i) => max(min(self.rows.len() as i32 - 1, i as i32 + 1), 0),
            None => 0,
        } as usize;

        self.state.select(Some(i));
    }

    fn prev_row(&mut self) {
        let i = match self.state.selected() {
            Some(i) => max(0, (i as i32) - 1),
            None => 0,
        } as usize;

        self.state.select(Some(i));
    }

    /// handle_key_event handles key events for the help menu.
    /// Returns true when user hit quit (Ctrl + Q).
    pub fn handle_key_event(&mut self, event: &Event) -> bool {
        match event {
            Event::KeyboardF1 => {
                self.showing = !self.showing;
                return false;
            }

            Event::Quit => {
                self.showing = false;
                return true;
            }

            Event::KeyboardCharInput(input) => match input.key {
                Key::Char('j') => self.next_row(),
                Key::Char('k') => self.prev_row(),
                Key::Char('q') => {
                    self.showing = false;
                    return false;
                }
                _ => {}
            },

            Event::UiScrollDown => self.next_row(),
            Event::UiScrollUp => self.prev_row(),

            _ => {}
        }

        false
    }

    pub fn render(&mut self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        if !self.showing {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightBlue))
            .padding(Padding::symmetric(1, 0))
            .title(Line::from(" Help ").bold())
            .title_alignment(Alignment::Center)
            .title_bottom(Line::from(vec![
                " ".into(),
                span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "q"),
                " to close ".into(),
                span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "↑/k/↓/j"),
                " to move up/down ".into(),
            ]))
            .style(Style::default());
        frame.render_widget(Clear, area);

        if self.last_known_width != area.width as usize
            || self.last_know_height != area.height as usize
        {
            self.last_known_width = area.width as usize;
            self.last_know_height = area.height as usize;

            self.rows = build_rows((area.width as f32 * 0.75).ceil() as usize);
            let row_index = 0;
            self.state.select(Some(row_index));
        }

        let selected_row_style = Style::default().add_modifier(Modifier::REVERSED);

        let table = Table::new(
            self.rows.clone(),
            [Constraint::Percentage(25), Constraint::Percentage(75)],
        )
        .block(block)
        .row_highlight_style(selected_row_style)
        .cell_highlight_style(Style::default().bg(Color::White));

        frame.render_stateful_widget(table, area, &mut self.state);
    }

    pub fn render_help_line(&self, frame: &mut ratatui::Frame, area: Rect) {
        let mut instructions = KEY_BINDINGS
            .iter()
            .filter(|b| !b.short_description.is_empty())
            .map(|b| {
                let key = b.key().to_string();
                let desc = b.short_description.clone();
                vec![
                    span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); key),
                    " ".into(),
                    span!(Style::default().fg(Color::White); desc),
                    " | ".into(),
                ]
            })
            .flatten()
            .collect::<Vec<_>>();
        instructions.pop(); // remove the last " | "

        let line = Line::from(instructions).light_green();
        frame.render_widget(line, area);
    }
}

fn build_rows<'a>(max_width: usize) -> Vec<Row<'a>> {
    let mut rows = vec![];
    for binding in KEY_BINDINGS.iter() {
        let key = Cell::from(binding.key().to_string()).style(Style::default());
        let desc = helpers::split_to_lines(
            binding
                .long_description()
                .split(' ')
                .map(|s| s.to_string().into())
                .collect(),
            max_width,
        );
        rows.push(Row::new(vec![key, Cell::from(Text::from(desc))]).height(ROW_HEIGHT as u16));
    }
    rows
}

fn build_key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new(Input::new(Key::Char('q')), "Close Popup"),
        KeyBinding::new(Input::new(Key::F(1)), "Show Help").with_short_desc("Help"),
        KeyBinding::new(Input::new(Key::Char('h')).ctrl(), "Show Chat [H]istory")
            .with_short_desc("History"),
        KeyBinding::new(Input::new(Key::Char('q')).ctrl(), "[Q]uit").with_short_desc("Quit"),
        KeyBinding::new(
            Input::new(Key::Char('c')).ctrl(),
            "Abort Request/[C]lear Chat",
        ),
        KeyBinding::new(Input::new(Key::Char('r')).ctrl(), "[R]egenerate Response"),
        KeyBinding::new(Input::new(Key::Char('m')).ctrl(), "[L]ist/Select Model"),
        KeyBinding::new(Input::new(Key::Char('e')).ctrl(), "[E]dit Mode"),
        KeyBinding::new(Input::new(Key::Char('n')).ctrl(), "[N]ew Chat"),
        KeyBinding::new(Input::new(Key::Up), "Scroll Up"),
        KeyBinding::new(Input::new(Key::Down), "Scroll Down"),
        KeyBinding::new(Input::new(Key::Up).ctrl(), "Scroll Page Up"),
        KeyBinding::new(Input::new(Key::Down).ctrl(), "Scroll Page Down"),
    ]
}

pub struct Input {
    key: Key,
    shift: bool,
    ctrl: bool,
    alt: bool,
}

impl Input {
    pub fn new(key: Key) -> Self {
        Self {
            key,
            shift: false,
            ctrl: false,
            alt: false,
        }
    }

    pub fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    pub fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    pub fn alt(mut self) -> Self {
        self.alt = true;
        self
    }
}

pub struct KeyBinding {
    key: Input,
    long_description: String,
    short_description: String,
}

impl KeyBinding {
    fn new(key: Input, description: &str) -> Self {
        Self {
            key,
            long_description: description.to_string(),
            short_description: String::new(),
        }
    }

    fn with_short_desc(mut self, short_description: &str) -> Self {
        self.short_description = short_description.to_string();
        self
    }

    pub fn key(&self) -> &Input {
        &self.key
    }

    pub fn long_description(&self) -> &str {
        &self.long_description
    }

    pub fn short_description(&self) -> &str {
        &self.short_description
    }
}

impl Display for Input {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut modifer = String::new();

        if self.ctrl {
            modifer.push_str("Ctrl+");
        }

        if self.alt {
            modifer.push_str("Alt+");
        }

        let key = match self.key {
            Key::Char(c) => {
                if self.shift {
                    c.to_uppercase().to_string()
                } else {
                    c.to_string()
                }
            }
            Key::F(n) => format!("F{}", n),
            Key::Backspace => "Backspace".to_string(),
            Key::Enter => "Enter".to_string(),
            Key::Left => "Left".to_string(),
            Key::Right => "Right".to_string(),
            Key::Up => "Up".to_string(),
            Key::Down => "Down".to_string(),
            Key::Tab => "Tab".to_string(),
            Key::Delete => "Delete".to_string(),
            Key::Home => "Home".to_string(),
            Key::End => "End".to_string(),
            Key::PageUp => "PageUp".to_string(),
            Key::PageDown => "PageDown".to_string(),
            Key::Esc => "Esc".to_string(),
            _ => "Unknown".to_string(),
        };

        write!(f, "{}{}", modifer, key)
    }
}
