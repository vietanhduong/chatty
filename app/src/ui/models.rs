use std::cmp::{max, min};

use eyre::{Context, Result};
use openai_backend::ArcBackend;
use openai_models::Event;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Cell, Clear, Padding, Row, Table, TableState},
};
use tui_textarea::Key;

use super::helpers;

pub struct ModelsScreen {
    showing: bool,
    models: Vec<String>,
    current_model: String,
    backend: ArcBackend,
    state: TableState,
    fetched: bool,
}

impl ModelsScreen {
    pub fn new(backend: ArcBackend) -> ModelsScreen {
        ModelsScreen {
            showing: false,
            backend,
            state: TableState::default().with_selected(0),
            models: Vec::new(),
            current_model: String::new(),
            fetched: false,
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

    async fn set_model(&mut self) -> Result<()> {
        let index = self.state.selected().unwrap_or(0);
        if index >= self.models.len() {
            return Ok(());
        }

        let selected = self.models[index].clone();

        let mut lock = self.backend.lock().await;
        if lock.current_model() == selected {
            return Ok(());
        }

        lock.set_model(&selected).await.wrap_err("setting model")?;
        self.current_model = selected;
        Ok(())
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if !self.showing {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightBlue))
            .padding(Padding::symmetric(1, 0))
            .title(" Models ")
            .title_alignment(Alignment::Center)
            .title_bottom(" <Esc>/<Space> to close/select ")
            .style(Style::default());
        let area = helpers::popup_area(area, 30, 60);
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

    pub async fn fetch_models(&mut self) -> Result<()> {
        let lock = self.backend.lock().await;
        self.current_model = lock.current_model().to_string();
        if self.fetched {
            return Ok(());
        }

        let models = lock.list_models().await.wrap_err("fetching models")?;
        self.models = models;
        self.fetched = true;
        Ok(())
    }

    pub async fn handle_key_event(&mut self, event: Event) -> Result<bool> {
        self.fetch_models().await?;

        match event {
            Event::KeyboardEsc => {
                self.showing = false;
                return Ok(false);
            }

            Event::KeyboardCtrlL => {
                self.showing = !self.showing;
                return Ok(false);
            }

            Event::KeyboardCtrlQ => {
                self.showing = false;
                return Ok(true);
            }

            Event::KeyboardCharInput(input) => match input.key {
                Key::Char('j') => self.next_row(),
                Key::Char('k') => self.prev_row(),
                Key::Char(' ') => self.set_model().await?,
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
            spans.push(Span::styled(
                if current { "[x] " } else { "[ ] " },
                Style::default(),
            ));
            spans.push(Span::styled(model, Style::default()));
            Row::new(vec![Cell::from(Text::from(Line::from(spans)))]).height(1)
        })
        .collect()
}
