use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{Block, BorderType, Borders, Clear, Padding},
};
use ratatui_macros::span;

use super::utils;

#[derive(Default)]
pub struct Question<'a> {
    showing: bool,
    question: Line<'a>,
    title: Option<Line<'a>>,
}

impl<'a> Question<'a> {
    pub fn with_title(mut self, title: impl Into<Line<'a>>) -> Question<'a> {
        self.set_title(title);
        self
    }

    pub fn set_title(&mut self, title: impl Into<Line<'a>>) {
        self.title = Some(title.into());
    }

    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn open(&mut self, question: impl Into<Line<'a>>) {
        self.question = question.into();
        self.showing = true;
    }

    pub fn close(&mut self) {
        self.showing = false;
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.showing {
            return;
        }

        let max_width = (area.width as f32 * 0.8).ceil() as u16;
        let lines = utils::split_to_lines(self.question.spans.clone(), (max_width - 2) as usize);
        let area = build_area(area, max_width, lines.len() as u16 + 2);

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::symmetric(1, 0))
            .title_bottom(vec![
                span!(" "),
                span!("q").green().bold(),
                span!(" to close, ").white(),
                span!("y").green().bold(),
                span!(" to confirm, ").white(),
                span!("n").green().bold(),
                span!(" to cancel ").white(),
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
}

fn build_area(area: Rect, w: u16, h: u16) -> Rect {
    let x = area.x + (area.width - w) / 2;
    let y = area.y + (area.height - 1) / 3;
    Rect::new(x, y, w, h)
}
