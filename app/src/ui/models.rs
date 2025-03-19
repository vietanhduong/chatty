use std::cmp::{max, min};

use eyre::Result;
use openai_models::{Action, Event};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Cell, Clear, Padding, Row, Table, TableState},
};
use ratatui_macros::span;
use tokio::sync::mpsc;
use tui_textarea::Key;

pub struct ModelsScreen {
    action_tx: mpsc::UnboundedSender<Action>,
    showing: bool,
    models: Vec<String>,
    current_model: String,
    state: TableState,
}

impl ModelsScreen {
    pub fn new(
        default_model: String,
        models: Vec<String>,
        action_tx: mpsc::UnboundedSender<Action>,
    ) -> ModelsScreen {
        ModelsScreen {
            showing: false,
            state: TableState::default().with_selected(0),
            models,
            current_model: default_model,
            action_tx,
        }
    }

    pub fn current_model(&self) -> &str {
        &self.current_model
    }

    pub fn set_current_model(&mut self, model: &str) {
        self.current_model = model.to_string();
    }

    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn toggle_showing(&mut self) {
        self.showing = !self.showing;
    }

    fn next_row(&mut self) {
        let i = match self.state.selected() {
            Some(i) => max(min(self.models.len() as i32 - 1, i as i32 + 1), 0),
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

    fn request_change_model(&mut self) -> Result<()> {
        let index = self.state.selected().unwrap_or(0);
        if index >= self.models.len() {
            return Ok(());
        }

        if self.current_model == self.models[index] {
            return Ok(());
        }

        self.action_tx
            .send(Action::BackendSetModel(self.models[index].to_string()))?;

        Ok(())
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if !self.showing {
            return;
        }

        let instructions = vec![
            " ".into(),
            span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "Esc/q"),
            " to close, ".into(),
            span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "Enter"),
            " to select, ".into(),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightBlue))
            .padding(Padding::symmetric(1, 0))
            .title(" Models ")
            .title_alignment(Alignment::Center)
            .title_bottom(Line::from(instructions))
            .style(Style::default());
        frame.render_widget(Clear, area);

        let selected_row_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .add_modifier(Modifier::BOLD);
        let rows = build_rows(&self.models, &self.current_model);
        let table = Table::new(rows, [Constraint::Fill(1)])
            .block(block)
            .row_highlight_style(selected_row_style);
        frame.render_stateful_widget(table, area, &mut self.state);
    }

    pub async fn handle_key_event(&mut self, event: &Event) -> Result<bool> {
        match event {
            Event::KeyboardEsc => {
                self.showing = false;
                return Ok(false);
            }

            Event::KeyboardCtrlL => {
                self.showing = !self.showing;
                return Ok(false);
            }

            Event::Quit => {
                self.showing = false;
                return Ok(true);
            }

            Event::ModelChanged(model) => {
                self.current_model = model.clone();
                return Ok(false);
            }

            Event::KeyboardEnter => {
                self.request_change_model()?;
                self.showing = false;
                return Ok(false);
            }

            Event::KeyboardCharInput(input) => match input.key {
                Key::Char('j') => self.next_row(),
                Key::Char('k') => self.prev_row(),
                Key::Char(' ') => self.request_change_model()?,
                Key::Char('q') => {
                    self.showing = false;
                    return Ok(false);
                }
                _ => {}
            },

            Event::UiScrollDown => self.next_row(),
            Event::UiScrollUp => self.prev_row(),
            _ => {}
        }

        Ok(false)
    }
}

fn build_rows<'a>(models: &'a [String], current_model: &str) -> Vec<Row<'a>> {
    models
        .iter()
        .map(|model| {
            let current = model == current_model;
            let mut spans = vec![];
            let mut style = Style::default();
            let mut text = "[ ]";
            if current {
                style = style.add_modifier(Modifier::BOLD).red();
                text = "[*]";
            }

            spans.push(Span::styled(text, style));
            spans.push(Span::styled(" ", Style::default()));
            spans.push(Span::styled(model, Style::default()));
            Row::new(vec![Cell::from(Text::from(Line::from(spans)))]).height(1)
        })
        .collect()
}
