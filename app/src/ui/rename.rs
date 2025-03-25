use openai_models::Event;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Padding, Widget},
};
use tui_textarea::{CursorMove, TextArea};

#[derive(Default)]
pub struct Rename<'a> {
    showing: bool,
    text: String,
    input: TextArea<'a>,
}

impl<'a> Rename<'a> {
    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn open(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.input = build_input(&self.text);
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
}

pub fn rename_area(area: Rect, width: u16) -> Rect {
    let x = area.x + (area.width - width) / 2;
    let y = area.y + (area.height - 1) / 2;
    Rect::new(x, y, width, 3)
}

fn build_input<'a>(text: &str) -> TextArea<'a> {
    let mut text_area = TextArea::new(vec![text.to_string()]);
    let block = Block::default()
        .title(Line::from(" Rename ").bold())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightMagenta))
        .padding(Padding::symmetric(1, 0));
    text_area.set_block(block);
    text_area.set_placeholder_text("Enter new name...");
    text_area.move_cursor(CursorMove::End);
    text_area
}
