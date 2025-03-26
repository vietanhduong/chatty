use crate::models::Event;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Padding, Widget},
};
use tui_textarea::{CursorMove, TextArea};

pub struct InputBox<'a> {
    showing: bool,
    text: String,
    input: TextArea<'a>,

    title: String,
    placeholder: String,
}

impl<'a> InputBox<'a> {
    pub fn with_title(mut self, title: &str) -> InputBox<'a> {
        self.set_title(title);
        self
    }

    pub fn with_placeholder(mut self, placeholder: &str) -> InputBox<'a> {
        self.set_placeholder(placeholder);
        self
    }

    pub fn set_title(&mut self, title: &str) {
        if !title.is_empty() {
            self.title = title.to_string();
        }
    }

    pub fn set_placeholder(&mut self, placeholder: &str) {
        if !placeholder.is_empty() {
            self.placeholder = placeholder.to_string();
        }
    }

    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn open(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.input = self.build_input();
        self.showing = true;
    }

    pub fn close(&mut self) -> Option<String> {
        if self.showing {
            self.showing = false;
            let text = self.input.lines().join("\n");
            return Some(text);
        }
        None
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.showing {
            return;
        }

        f.render_widget(Clear, area);
        self.input.render(area, f.buffer_mut());
    }

    pub fn handle_key_event(&mut self, event: &Event) {
        match event {
            Event::KeyboardCharInput(input) => {
                self.input.input(input.clone());
            }
            _ => {}
        }
    }

    fn build_input(&self) -> TextArea<'a> {
        let mut text_area = TextArea::new(vec![self.text.clone()]);
        let block = Block::default()
            .title(Line::from(self.title.clone()).bold())
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightMagenta))
            .padding(Padding::symmetric(1, 0));
        text_area.set_block(block);
        text_area.set_placeholder_text(&self.placeholder);
        text_area.move_cursor(CursorMove::End);
        text_area
    }
}

impl Default for InputBox<'_> {
    fn default() -> Self {
        Self {
            showing: false,
            text: String::new(),
            input: TextArea::default(),
            title: "Input".to_string(),
            placeholder: "Type here...".to_string(),
        }
    }
}

pub fn build_area(area: Rect, width: u16) -> Rect {
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - 1) / 2;
    Rect::new(x, y, width, 3)
}
