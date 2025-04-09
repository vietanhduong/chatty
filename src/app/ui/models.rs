use std::collections::{BTreeMap, HashMap};

use crate::{
    config::{self},
    info_event,
    models::{Event, Model},
};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding},
};
use ratatui_macros::span;
use tokio::sync::mpsc;
use tui_textarea::Key;

use super::{
    Dim,
    input_box::{self, InputBox},
    utils,
};

pub struct ModelsScreen<'a> {
    event_tx: mpsc::UnboundedSender<Event>,

    showing: bool,
    models: Vec<Model>,
    idx_map: HashMap<usize, String>,

    current_model: String,
    state: ListState,
    items: Vec<ListItem<'a>>,

    last_known_width: usize,

    search: InputBox<'a>,
    current_search: String,
}

impl<'a> ModelsScreen<'a> {
    pub fn new(models: Vec<Model>, event_tx: mpsc::UnboundedSender<Event>) -> ModelsScreen<'a> {
        let want_model = config::instance()
            .backend
            .default_model
            .as_deref()
            .unwrap_or_default();

        let default_model = models
            .iter()
            .find(|model| want_model == model.id())
            .unwrap_or_else(|| &models[0])
            .id()
            .to_string();

        ModelsScreen {
            event_tx,
            showing: false,
            models,
            current_model: default_model,
            search: InputBox::default().with_title(" Search "),
            current_search: String::new(),
            last_known_width: 0,

            state: ListState::default(),
            idx_map: HashMap::new(),
            items: vec![],
        }
    }

    pub fn current_model(&self) -> &str {
        &self.current_model
    }

    pub fn set_current_model(&mut self, model: &str) {
        if self.current_model == model {
            return;
        }
        self.current_model = model.to_string();

        let _ = self
            .event_tx
            .send(info_event!(format!("Model changed to \"{}\"", model)));

        self.build_items();
        self.set_cursor_to_selected();
    }

    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn toggle_showing(&mut self) {
        self.showing = !self.showing;
    }

    fn next_row(&mut self) {
        if self.models.is_empty() {
            self.state.select(None);
            return;
        }

        let i = match self.state.selected() {
            Some(i) => (i + 1).min(self.items.len() - 1),
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn prev_row(&mut self) {
        if self.models.is_empty() {
            self.state.select(None);
            return;
        }

        let i = match self.state.selected() {
            Some(i) => (i as isize - 1).max(0) as usize,
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn first(&mut self) {
        if self.models.is_empty() {
            self.state.select(None);
            return;
        }
        self.state.select(Some(0));
        // if the first item is a group header, we need to select the next item
        self.next_row();
    }

    fn last(&mut self) {
        if self.models.is_empty() {
            self.state.select(None);
            return;
        }
        self.state.select(Some(self.items.len() - 1));
    }

    fn request_change_model(&mut self) -> bool {
        let index = self.state.selected().unwrap_or(0);
        if index >= self.models.len() {
            return false;
        }

        let model = match self.idx_map.get(&index) {
            Some(idx) => idx,
            None => return false,
        };

        if self.current_model == *model {
            return false;
        }

        let model = model.to_string();
        self.set_current_model(&model);

        true
    }

    fn set_cursor_to_selected(&mut self) {
        if let Some(item) = self
            .idx_map
            .iter()
            .find(|(_, model)| **model == self.current_model)
        {
            self.state.select(Some(*item.0));
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.showing {
            return;
        }

        f.dim_bg();

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

        if self.last_known_width != inner.width as usize {
            let prev_width = self.last_known_width;

            self.last_known_width = inner.width as usize;
            self.build_items();
            // Only set the cursor to the selected item at the first time
            if prev_width == 0 {
                self.set_cursor_to_selected();
            }
        }

        let list = List::new(self.items.clone())
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        f.render_stateful_widget(list, area, &mut self.state);

        let search_area = input_box::build_area(inner, ((inner.width as f32 * 0.9).ceil()) as u16);
        self.search.render(f, search_area);
    }

    pub async fn handle_key_event(&mut self, event: &Event) -> bool {
        if self.search.showing() {
            self.handle_search_popup(event).await;
            return false;
        }

        match event {
            Event::KeyboardCtrlL => {
                self.showing = !self.showing;
            }

            Event::Quit => {
                self.showing = false;
                return true;
            }

            Event::KeyboardEnter => {
                self.showing = !self.request_change_model();
            }

            Event::KeyboardCharInput(input) => match input.key {
                Key::Char('j') => self.next_row(),
                Key::Char('k') => self.prev_row(),
                Key::Char('g') => self.first(),
                Key::Char('G') => self.last(),
                Key::Char('/') => self.search.open(&self.current_search),
                Key::Char('q') => {
                    self.showing = false;
                }
                _ => {}
            },

            Event::UiScrollDown => self.next_row(),
            Event::UiScrollUp => self.prev_row(),
            _ => {}
        }

        false
    }

    async fn handle_search_popup(&mut self, event: &Event) {
        match event {
            Event::KeyboardEsc | Event::KeyboardCtrlC => {
                self.search.close();
            }
            Event::KeyboardEnter => {
                self.current_search = self.search.close().unwrap_or_default();
                self.build_items();
                if !self.items.is_empty() {
                    self.state.select(Some(0));
                }
            }
            _ => self.search.handle_key_event(event),
        }
    }

    fn build_items(&mut self) {
        self.idx_map.clear();
        self.items.clear();

        let mut models: BTreeMap<String, Vec<String>> = BTreeMap::new();

        self.models
            .iter()
            .filter(|model| {
                if self.current_search.is_empty() {
                    return true;
                }
                model
                    .id()
                    .to_lowercase()
                    .contains(&self.current_search.to_lowercase())
            })
            .for_each(|m| {
                let model = m.id().to_string();
                let alias = m.provider().to_string();
                models.entry(alias).or_default().push(model);
            });

        for (provider, models) in models {
            self.items.push(header_item(provider));

            for model in models {
                let mut spans = vec![span!(model)];
                if self.current_model == model {
                    spans.push(Span::styled(" ", Style::default()));
                    spans.push(Span::styled("[*]", Style::default().fg(Color::LightRed)))
                }

                let lines = utils::split_to_lines(spans, self.last_known_width - 2);
                self.items.push(ListItem::new(Text::from(lines)));
                self.idx_map.insert(self.items.len() - 1, model);
            }
        }
    }
}

fn header_item<'a>(value: String) -> ListItem<'a> {
    ListItem::new(Text::from(value).alignment(Alignment::Center).bold())
        .style(
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::Rgb(26, 35, 126)),
        )
        .add_modifier(Modifier::BOLD)
}
