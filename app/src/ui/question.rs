use std::pin::Pin;

use openai_models::Event;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, BorderType, Borders, Clear, Padding},
};
use ratatui_macros::span;
use tui_textarea::Key;

use super::helpers;

pub struct Question<'a> {
    showing: bool,
    question: Line<'a>,
    title: Option<Line<'a>>,
    answer_callback: Option<Box<dyn Fn(bool) -> Pin<Box<dyn Future<Output = ()>>> + 'a>>,
}

impl<'a> Question<'a> {
    pub fn new() -> Question<'a> {
        Question {
            title: None,
            showing: false,
            question: Line::default(),
            answer_callback: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<Line<'a>>) -> Question<'a> {
        self.set_title(title);
        self
    }

    pub fn set_title(&mut self, title: impl Into<Line<'a>>) {
        self.title = Some(title.into());
    }

    pub fn set_question(&mut self, question: impl Into<Line<'a>>) {
        self.question = question.into();
    }

    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(bool) -> Pin<Box<dyn Future<Output = ()>>> + 'a,
    {
        self.answer_callback = Some(Box::new(callback));
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

        let max_width = (area.width as f32 * 0.8).ceil() as u16;
        let lines = helpers::split_to_lines(self.question.spans.clone(), (max_width - 2) as usize);
        let area = build_area(area, max_width, lines.len() as u16 + 2);

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::symmetric(1, 0))
            .title_bottom(vec![
                span!(" "),
                span!("q").green().bold(),
                span!(" to close, "),
                span!("y").green().bold(),
                span!(" to confirm, "),
                span!("n").green().bold(),
                span!(" to cancel "),
            ])
            .title_alignment(Alignment::Center)
            .border_style(Style::default().light_blue());

        if let Some(title) = &self.title {
            block = block
                .title(title.clone())
                .title_alignment(Alignment::Center);
        }

        f.render_widget(Clear, area);
        let inner = block.inner(area);
        f.render_widget(block, area);

        let text = Text::from(lines);
        f.render_widget(text, inner);
    }

    pub async fn handle_key_event(&mut self, event: &Event) {
        match event {
            Event::KeyboardCharInput(input) => match input.key {
                Key::Char('q') => {
                    self.showing = false;
                }

                Key::Char('y') => {
                    if let Some(callback) = &self.answer_callback {
                        callback(true).await;
                    }
                    self.showing = false;
                }

                Key::Char('n') => {
                    if let Some(callback) = &self.answer_callback {
                        callback(false).await;
                    }
                    self.showing = false;
                }
                _ => {}
            },
            _ => {}
        }
    }
}

fn build_area(area: Rect, w: u16, h: u16) -> Rect {
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - 1) / 3;
    Rect::new(x, y, w, h)
}
