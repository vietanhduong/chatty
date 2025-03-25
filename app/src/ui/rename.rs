use std::pin::Pin;

use openai_models::Event;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Clear, Padding, Widget},
};
use tui_textarea::{CursorMove, TextArea};

#[derive(Default)]
pub struct Rename<'a> {
    showing: bool,
    text: String,
    input: TextArea<'a>,
    callback: Option<Box<dyn Fn(&str) -> Pin<Box<dyn Future<Output = ()>>> + 'a>>,
}

impl<'a> Rename<'a> {
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.input = build_input(text);
    }

    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str) -> Pin<Box<dyn Future<Output = ()>>> + 'a,
    {
        self.callback = Some(Box::new(callback));
    }

    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn toggle_showing(&mut self) {
        self.showing = !self.showing;
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.showing {
            return;
        }

        f.render_widget(Clear, area);
        self.input.render(area, f.buffer_mut());
    }

    pub async fn handle_key_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Quit => {
                self.showing = false;
                return true;
            }
            Event::KeyboardEsc | Event::KeyboardCtrlC => {
                self.showing = false;
            }
            Event::KeyboardCharInput(input) => {
                self.input.input(input.clone());
            }
            Event::KeyboardEnter => {
                self.text = self.input.lines().join("\n");
                if let Some(callback) = &self.callback {
                    callback(&self.text).await;
                }
                self.showing = false;
            }
            _ => {}
        }
        return false;
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
        .title("Rename")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightMagenta))
        .padding(Padding::symmetric(1, 0));
    text_area.set_block(block);
    text_area.set_placeholder_text("Enter new name...");
    text_area.move_cursor(CursorMove::End);
    text_area
}
