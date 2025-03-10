use once_cell::sync::Lazy;
use openai_models::Event;
use std::{
    cmp::{max, min},
    fmt::Display,
};

use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{Block, Cell, Clear, Row, Table, TableState},
};
use tui_textarea::Key;

pub const KEY_BINDINGS: Lazy<Vec<KeyBinding>> = Lazy::new(build_key_bindings);
const ROW_HEIGHT: usize = 1;

pub struct Help<'a> {
    showing: bool,

    state: TableState,

    rows: Vec<Row<'a>>,
    last_known_width: usize,
    last_know_height: usize,
    up_direction: bool,
}

impl<'a> Help<'_> {
    pub fn new() -> Help<'a> {
        Help {
            showing: false,
            state: TableState::default().with_selected(0),
            rows: vec![],
            last_known_width: 0,
            last_know_height: 0,
            up_direction: false,
        }
    }

    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn toggle_showing(&mut self) {
        self.showing = !self.showing;
    }

    fn next_row(&mut self) {
        let step = if self.up_direction {
            self.up_direction = false;
            self.last_know_height / ROW_HEIGHT - 2
        } else {
            1
        } as i32;
        let mut i = match self.state.selected() {
            Some(i) => max(min(self.rows.len() as i32 - 1, i as i32 + step), 0),
            None => 0,
        } as usize;

        let max_top = self.max_top_index();

        if i <= max_top {
            i = max_top + 1;
        }
        self.state.select(Some(i));
    }

    fn prev_row(&mut self) {
        let step = if !self.up_direction {
            self.up_direction = true;
            self.last_know_height / ROW_HEIGHT - 2
        } else {
            1
        } as i32;
        let mut i = match self.state.selected() {
            Some(i) => max(0, (i as i32) - step),
            None => 0,
        } as usize;

        let max_bot = self.max_bot_index();

        if i >= max_bot {
            i = max_bot - 1;
        }

        self.state.select(Some(i));
    }

    /// max_top_row returns the maximum top row index that can be displayed
    /// in the current viewport. This is used to determine the maximum
    /// scroll position for the scrollbar.
    fn max_top_index(&self) -> usize {
        (((self.last_know_height as usize - 2) / ROW_HEIGHT) as i32 - 1) as usize
    }

    fn max_bot_index(&self) -> usize {
        self.rows.len() - ((self.last_know_height - 2) / ROW_HEIGHT)
    }

    /// handle_key_event handles key events for the help menu.
    /// Returns true when user hit quit (Ctrl + Q).
    pub fn handle_key_event(&mut self, event: Event) -> bool {
        match event {
            Event::KeyboardEsc => {
                self.showing = false;
            }

            Event::KeyboardF1 => {
                self.showing = !self.showing;
                return false;
            }

            Event::KeyboardCtrlQ => {
                self.showing = false;
                return true;
            }

            Event::KeyboardCharInput(input) => {
                if Key::Char('q') == input.key {
                    self.showing = false;
                    return false;
                }
            }
            Event::UiScrollDown => self.next_row(),
            Event::UiScrollUp => self.prev_row(),

            _ => {}
        }

        false
    }

    pub fn render(&mut self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let block = Block::bordered()
            .title(" Help ")
            .title_alignment(Alignment::Center)
            .title_bottom("<Esc>/<q> to close")
            .style(Style {
                bg: Some(Color::Blue),
                fg: Some(Color::White),
                ..Default::default()
            });
        let area = Self::popup_area(area, 60, 30);
        frame.render_widget(Clear, area);

        if self.last_known_width != area.width as usize
            || self.last_know_height != area.height as usize
        {
            self.last_known_width = area.width as usize;
            self.last_know_height = area.height as usize;

            self.rows = build_rows((area.width as f32 * 0.75).ceil() as usize);
            let row_index = self.max_top_index();
            self.state.select(Some(row_index));
        }

        let table = Table::new(
            self.rows.clone(),
            [Constraint::Percentage(25), Constraint::Percentage(75)],
        )
        .block(block);

        frame.render_stateful_widget(table, area, &mut self.state);
    }

    pub fn render_help_line(&self, frame: &mut ratatui::Frame, area: Rect) {
        let instructions = KEY_BINDINGS
            .iter()
            .filter(|b| !b.short_description.is_empty())
            .map(|b| format!("{}: {}", b.key(), b.short_description()))
            .collect::<Vec<_>>();
        let line = Line::from(instructions.join(" | ")).blue();
        frame.render_widget(line, area);
    }

    fn popup_area(area: Rect, percent_width: u16, percent_height: u16) -> Rect {
        let vertical =
            Layout::vertical([Constraint::Percentage(percent_height)]).flex(Flex::Center);
        let horizontal =
            Layout::horizontal([Constraint::Percentage(percent_width)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        area
    }
}

fn build_rows<'a>(max_width: usize) -> Vec<Row<'a>> {
    let mut rows = vec![];
    for binding in KEY_BINDINGS.iter() {
        let key = Cell::from(binding.key().to_string()).style(Style {
            fg: Some(Color::Yellow),
            ..Default::default()
        });

        let mut line = String::new();
        let mut first = false;
        for word in binding.long_description().split(' ') {
            if line.len() + word.len() > max_width - 2 {
                rows.push(
                    Row::new(vec![
                        if !first { key.clone() } else { Cell::from("") },
                        Cell::from(Text::from(line.trim().to_string())),
                    ])
                    .height(ROW_HEIGHT as u16),
                );
                line.clear();
                first = true;
            }
            line.push_str(word);
            line.push(' ');
        }
        if !line.is_empty() {
            rows.push(
                Row::new(vec![
                    if !first { key.clone() } else { Cell::from("") },
                    Cell::from(Text::from(line.trim().to_string())),
                ])
                .height(ROW_HEIGHT as u16),
            );
        }
    }
    rows
}

fn build_key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new(Input::new(Key::Char('h')).ctrl(), "Show Chat History")
            .with_short_desc("History"),
        KeyBinding::new(Input::new(Key::Char('q')).ctrl(), "Quit").with_short_desc("Quit"),
        KeyBinding::new(Input::new(Key::F(1)), "Show Help").with_short_desc("Help"),
        KeyBinding::new(Input::new(Key::Char('c')).ctrl(), "Abort Request"),
        KeyBinding::new(Input::new(Key::Char('r')).ctrl(), "Regenerate Response"),
        KeyBinding::new(Input::new(Key::Char('m')).ctrl(), "Select Model"),
        KeyBinding::new(Input::new(Key::Char('n')).ctrl(), "New Chat"),
        KeyBinding::new(Input::new(Key::Up).ctrl(), "Scroll Up"),
        KeyBinding::new(Input::new(Key::Down).ctrl(), "Scroll Down"),
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

        write!(f, "<{}{}>", modifer, key)
    }
}
