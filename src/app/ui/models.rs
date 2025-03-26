use std::cmp::{max, min};

use crate::models::{Action, Event};
use eyre::Result;
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

use super::input_box::{self, InputBox};

pub struct ModelsScreen<'a> {
    action_tx: mpsc::UnboundedSender<Action>,
    showing: bool,
    models: Vec<String>,
    current_model: String,
    state: TableState,

    search: InputBox<'a>,
    current_search: String,
}

impl<'a> ModelsScreen<'a> {
    pub fn new(
        default_model: String,
        models: Vec<String>,
        action_tx: mpsc::UnboundedSender<Action>,
    ) -> ModelsScreen<'a> {
        ModelsScreen {
            showing: false,
            state: TableState::default().with_selected(0),
            models,
            current_model: default_model,
            action_tx,
            search: InputBox::default().with_title(" Search "),
            current_search: String::new(),
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

        let models = self
            .models
            .iter()
            .filter(|model| {
                if self.current_search.is_empty() {
                    return true;
                }
                model
                    .to_lowercase()
                    .contains(&self.current_search.to_lowercase())
            })
            .collect::<Vec<_>>();

        if self.current_model == *models[index] {
            return Ok(());
        }

        self.action_tx
            .send(Action::BackendSetModel(models[index].to_string()))?;

        Ok(())
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.showing {
            return;
        }

        let instructions = vec![
            " ".into(),
            span!("q").green().bold(),
            span!(" to close, ").white(),
            span!("Enter").green().bold(),
            span!(" to select, ").white(),
            span!("/").green().bold(),
            span!(" to search ").white(),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightBlue))
            .padding(Padding::symmetric(1, 0))
            .title(Line::from(" Models ").bold())
            .title_alignment(Alignment::Center)
            .title_bottom(Line::from(instructions))
            .style(Style::default());
        f.render_widget(Clear, area);

        let inner = block.inner(area);

        let selected_row_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .add_modifier(Modifier::BOLD);
        let rows = build_rows(&self.models, &self.current_model, &self.current_search);
        let table = Table::new(rows, [Constraint::Fill(1)])
            .block(block)
            .row_highlight_style(selected_row_style);
        f.render_stateful_widget(table, area, &mut self.state);
        let search_area = input_box::build_area(inner, ((inner.width as f32 * 0.9).ceil()) as u16);
        self.search.render(f, search_area);
    }

    pub async fn handle_key_event(&mut self, event: &Event) -> Result<bool> {
        if self.search.showing() {
            match event {
                Event::KeyboardEsc | Event::KeyboardCtrlC => {
                    self.search.close();
                }
                Event::KeyboardEnter => {
                    self.current_search = self.search.close().unwrap_or_default();
                }
                _ => self.search.handle_key_event(event),
            }

            return Ok(false);
        }

        match event {
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
                Key::Char('/') => {
                    self.search.open(&self.current_search);
                    return Ok(false);
                }
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

fn build_rows<'a>(models: &'a [String], current_model: &str, search_text: &str) -> Vec<Row<'a>> {
    models
        .iter()
        .filter(|model| {
            if search_text.is_empty() {
                return true;
            }
            model.to_lowercase().contains(&search_text.to_lowercase())
        })
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
