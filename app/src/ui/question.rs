use openai_models::Event;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
};
use tui_textarea::Key;

use super::helpers;

pub struct Question<'a> {
    showing: bool,
    question: String,
    answer_callback: Option<Box<dyn Fn(bool) + 'a>>,
}

impl<'a> Question<'a> {
    pub fn new() -> Question<'a> {
        Question {
            showing: false,
            question: String::new(),
            answer_callback: None,
        }
    }

    pub fn set_question(&mut self, question: impl Into<String>) {
        self.question = question.into();
    }

    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(bool) + 'a,
    {
        self.answer_callback = Some(Box::new(callback));
    }

    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn toggle_showing(&mut self) {
        self.showing = !self.showing;
    }

    pub fn question(&self) -> &str {
        &self.question
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.showing {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::symmetric(1, 0))
            .border_style(Style::default().fg(Color::LightBlue));

        f.render_widget(Clear, area);
        let inner = block.inner(area);
        f.render_widget(block, area);

        // --------Question-line---------
        // [ ] [Yes] [ No ] [ ]
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(3)].as_ref())
            .split(inner);

        let text = Text::from(helpers::split_to_lines(
            self.question
                .split(' ')
                .into_iter()
                .map(|s| Span::raw(s.to_string()))
                .collect(),
            layout[0].width as usize,
        ));

        f.render_widget(text, layout[0]);

        let answer_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 5); 5].as_ref())
            .split(layout[1]);

        let yes_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().fg(Color::LightGreen));

        let no_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().fg(Color::Red));

        let yes = Paragraph::new("Yes")
            .block(yes_block)
            .alignment(ratatui::layout::Alignment::Center);

        let no = Paragraph::new(" No")
            .block(no_block)
            .alignment(ratatui::layout::Alignment::Center);

        f.render_widget(yes, answer_layout[1]);
        f.render_widget(no, answer_layout[3]);
    }

    pub fn handle_key_event(&mut self, event: &Event) {
        match event {
            Event::KeyboardEsc => {
                self.showing = false;
            }
            Event::KeyboardCharInput(input) => match input.key {
                Key::Char('q') => {
                    self.showing = false;
                }

                Key::Char('y') => {
                    if let Some(callback) = &self.answer_callback {
                        callback(true);
                    }
                    self.showing = false;
                }

                Key::Char('n') => {
                    if let Some(callback) = &self.answer_callback {
                        callback(false);
                    }
                    self.showing = false;
                }
                _ => {}
            },
            _ => {}
        }
    }
}
