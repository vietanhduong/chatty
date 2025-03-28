use crate::models::{Conversation, Event, NoticeMessage};
use crate::storage::ArcStorage;
use chrono::{Local, Utc};
use eyre::Result;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding},
};
use ratatui_macros::span;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};
use tokio::sync::mpsc;
use tui_textarea::Key;

use super::input_box::{self, InputBox};
use super::{question::Question, utils};

const NO_CONVERSATIONS: &str = "No conversations found";

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ConversationGroup {
    Today,
    Yesterday,
    Last7Days,
    Last30Days,
    Older,
}

pub struct HistoryScreen<'a> {
    showing: bool,

    event_tx: mpsc::UnboundedSender<Event>,

    storage: ArcStorage,
    conversations: Vec<Conversation>,
    items: Vec<ListItem<'a>>,
    idx_map: HashMap<usize, String>,

    rename: InputBox<'a>,
    search: InputBox<'a>,
    current_search: String,

    question: Question<'a>,

    current_conversation: Option<String>,
    state: ListState,

    last_known_width: usize,
}

impl<'a> HistoryScreen<'a> {
    pub fn new(event_tx: mpsc::UnboundedSender<Event>, storage: ArcStorage) -> HistoryScreen<'a> {
        HistoryScreen {
            event_tx,
            storage,
            showing: false,
            conversations: vec![],

            idx_map: HashMap::new(),
            rename: InputBox::default().with_title(" Rename "),
            search: InputBox::default().with_title(" Search "),
            question: Question::new().with_title(" Delete Conversation "),

            current_search: String::new(),
            current_conversation: None,

            last_known_width: 0,

            items: vec![],
            state: ListState::default(),
        }
    }

    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn with_conversations(mut self, conversations: &[Conversation]) -> HistoryScreen<'a> {
        self.conversations = conversations.to_vec();
        // sort the conversations by last updated time descending
        self.conversations
            .sort_by(|a, b| b.updated_at().cmp(&a.updated_at()));
        self.conversations.dedup_by(|a, b| a.id() == b.id());
        self
    }

    pub fn with_current_conversation(
        mut self,
        current_conversation: impl Into<String>,
    ) -> HistoryScreen<'a> {
        self.current_conversation = Some(current_conversation.into());
        self
    }

    pub fn toggle_showing(&mut self) {
        self.showing = !self.showing;
        if self.showing && self.current_conversation.is_some() {
            self.move_cursor_to_current();
        }
    }

    pub fn remove_conversation(&mut self, conversation: &str) {
        if let Some(pos) = self
            .conversations
            .iter()
            .position(|c| c.id() == conversation)
        {
            if self.current_conversation.as_deref() == Some(conversation) {
                self.current_conversation = None;
            }
            self.conversations.remove(pos);
            self.update_items();
        }
    }

    fn move_cursor_to_current(&mut self) {
        if let Some(current_conversation) = self.current_conversation.as_ref() {
            let pos = self
                .idx_map
                .iter()
                .find(|(_, id)| *id == current_conversation)
                .map(|(pos, _)| *pos);
            self.state.select(pos);
        }
    }

    pub fn upsert_conversation(&mut self, conversation: &Conversation) {
        // If the conversation already exists, just update it
        // otherwise, add the conversation at the top of the list
        let pos = self
            .conversations
            .iter()
            .position(|c| c.id() == conversation.id())
            .unwrap_or_default();

        if pos != 0 {
            // remove the conversation from the list
            self.conversations.remove(pos);
        }

        self.conversations.insert(0, conversation.clone());
        // sort the conversations by last updated time descending
        self.conversations
            .sort_by(|a, b| b.updated_at().cmp(&a.updated_at()));
        self.update_items();
    }

    pub fn add_conversation_and_set(&mut self, conversation: &Conversation) {
        let id = conversation.id().to_string();
        self.upsert_conversation(conversation);
        self.current_conversation = Some(id);
    }

    pub fn set_current_conversation(&mut self, conversation: impl Into<String>) {
        self.current_conversation = Some(conversation.into());
        self.move_cursor_to_current();
        self.update_items();
    }

    fn next_row(&mut self) {
        if self.conversations.is_empty() {
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
        if self.conversations.is_empty() {
            self.state.select(None);
            return;
        }

        let i = match self.state.selected() {
            Some(i) => (i as isize - 1).max(0) as usize,
            None => 0,
        };

        self.state.select(Some(i));
    }

    fn pageup(&mut self) {
        for _ in 0..10 {
            self.prev_row();
        }
    }

    fn pagedown(&mut self) {
        for _ in 0..10 {
            self.next_row();
        }
    }

    fn first(&mut self) {
        if self.conversations.is_empty() {
            self.state.select(None);
            return;
        }
        self.state.select(Some(0));
        // if the first item is a group header, we need to select the next item
        self.next_row();
    }

    fn last(&mut self) {
        if self.conversations.is_empty() {
            self.state.select(None);
            return;
        }
        self.state.select(Some(self.items.len() - 1));
    }

    pub fn update_items(&mut self) {
        self.items.clear();
        self.idx_map.clear();

        if self.conversations.is_empty() {
            self.items.push(ListItem::new(
                Text::from(NO_CONVERSATIONS).alignment(Alignment::Center),
            ));
            self.state.select(None);
            return;
        }

        let mut conversations: BTreeMap<ConversationGroup, Vec<&Conversation>> = BTreeMap::new();

        let now = Utc::now();
        self.conversations
            .iter()
            .filter(|c| {
                if self.current_search.is_empty() {
                    return true;
                }
                c.title()
                    .to_lowercase()
                    .contains(&self.current_search.to_lowercase())
            })
            .for_each(|c| {
                let group = categorize_conversation(now, c.updated_at());

                conversations.entry(group).or_insert_with(Vec::new).push(c);
            });

        for (group, conversations) in conversations {
            self.items.push(group.to_list_item());

            for c in conversations {
                let mut spans = vec![span!(c.title())];
                if self.current_conversation.as_deref() == Some(c.id()) {
                    spans.push(Span::styled(" ", Style::default()));
                    spans.push(Span::styled("[*]", Style::default().fg(Color::LightRed)))
                }

                let lines = utils::split_to_lines(spans, self.last_known_width);
                self.items.push(ListItem::new(Text::from(lines)));
                self.idx_map
                    .insert(self.items.len() - 1, c.id().to_string());
            }
        }
    }

    pub async fn handle_key_event(&mut self, event: &Event) -> Result<bool> {
        if self.rename.showing() {
            self.handle_rename_popup(event).await;
            return Ok(false);
        }

        if self.question.showing() {
            self.handle_question_popup(event).await;
            return Ok(false);
        }

        if self.search.showing() {
            self.handle_search_popup(event).await;
            return Ok(false);
        }

        match event {
            Event::KeyboardCtrlH => {
                self.showing = !self.showing;
                return Ok(false);
            }
            Event::Quit => {
                self.showing = false;
                return Ok(true);
            }

            Event::KeyboardEnter => {
                if self.state.selected().is_none() || self.conversations.is_empty() {
                    return Ok(false);
                }

                let id = match self.get_selected_conversation_id() {
                    Some(id) => id.to_string(),
                    None => return Ok(false),
                };

                self.showing = false;
                self.event_tx.send(Event::SetConversation(id.clone())).ok();
                self.current_conversation = Some(id);
                return Ok(false);
            }

            Event::KeyboardCharInput(input) => match input.key {
                Key::Char('j') => self.next_row(),
                Key::Char('k') => self.prev_row(),
                Key::Char('g') => self.first(),
                Key::Char('G') => self.last(),

                Key::Char('q') => {
                    self.showing = false;
                }

                Key::Char('/') => self.search.open(self.current_search.clone()),

                Key::Char('d') => {
                    let conversation = match self.get_selected_conversation() {
                        Some(c) => c,
                        None => return Ok(false),
                    };

                    if conversation.id().is_empty() {
                        return Ok(false);
                    }

                    let quest = vec![
                        span!("Do you want to delete"),
                        span!(format!("\"{}\"", conversation.title()))
                            .add_modifier(Modifier::BOLD | Modifier::ITALIC)
                            .yellow(),
                        span!("?"),
                    ];
                    self.question.open(quest);
                }
                Key::Char('r') => {
                    if let Some(conversation) = self.get_selected_conversation() {
                        // Ignore with blank conversation
                        if conversation.id().is_empty() {
                            return Ok(false);
                        }
                        let title = conversation.title().to_string();
                        self.rename.open(title);
                    }
                }
                _ => {}
            },

            Event::UiScrollUp => self.prev_row(),
            Event::UiScrollDown => self.next_row(),
            Event::UiScrollPageUp => self.pageup(),
            Event::UiScrollPageDown => self.pagedown(),

            _ => {}
        }
        Ok(false)
    }

    async fn handle_search_popup(&mut self, event: &Event) {
        match event {
            Event::KeyboardEsc | Event::KeyboardCtrlC => {
                self.search.close();
            }
            Event::KeyboardEnter => {
                self.current_search = self.search.close().unwrap_or_default();
                self.update_items();
                if !self.items.is_empty() {
                    self.state.select(Some(0));
                }
            }
            _ => self.search.handle_key_event(event),
        }
    }

    async fn handle_question_popup(&mut self, event: &Event) {
        match event {
            Event::KeyboardCharInput(input) => match input.key {
                Key::Char('y') => {
                    self.on_delete().await;
                    self.question.close();
                }
                Key::Char('n') | Key::Char('q') => {
                    self.question.close();
                }
                _ => {}
            },
            _ => {}
        }
    }

    async fn handle_rename_popup(&mut self, event: &Event) {
        match event {
            Event::KeyboardEnter => {
                let text = self.rename.close().unwrap_or_default();
                self.rename_conversation(&text).await;
            }
            Event::KeyboardCtrlC | Event::KeyboardEsc => {
                self.rename.close();
            }
            _ => self.rename.handle_key_event(event),
        }
    }

    async fn on_delete(&mut self) {
        let convo_id = match self.get_selected_conversation_id() {
            Some(id) => id.to_string(),
            None => return,
        };

        if let Err(err) = self.storage.delete_conversation(&convo_id).await {
            log::error!("Failed to delete conversation: {}", err);
            self.event_tx
                .send(Event::Notice(NoticeMessage::warning(format!(
                    "Failed to delete conversation: {}",
                    err
                ))))
                .ok();
            return;
        }

        self.event_tx
            .send(Event::ConversationDeleted(convo_id))
            .ok();
    }

    pub async fn rename_conversation(&mut self, new_title: &str) {
        let convo_id = match self.get_selected_conversation_id() {
            Some(id) => id.to_string(),
            None => return,
        };

        let convo = match self.conversations.iter_mut().find(|c| c.id() == convo_id) {
            Some(c) => c,
            None => return,
        };

        if new_title.is_empty() {
            return;
        }
        {
            convo.set_title(new_title.to_string());
        }
        let convo = convo.clone();
        self.update_items();
        if let Err(err) = self.storage.upsert_conversation(convo).await {
            log::error!("Failed to rename conversation: {}", err);
            self.event_tx
                .send(Event::Notice(NoticeMessage::warning(format!(
                    "Failed to rename conversation: {}",
                    err
                ))))
                .ok();
        }
    }

    pub fn get_selected_conversation_id(&self) -> Option<&str> {
        if self.state.selected().is_none() || self.conversations.is_empty() {
            return None;
        }
        let idx = self.state.selected().unwrap();
        match self.idx_map.get(&idx) {
            Some(id) => Some(id),
            _ => None,
        }
    }

    pub fn get_selected_conversation(&self) -> Option<&Conversation> {
        let id = self.get_selected_conversation_id()?.to_string();
        self.conversations.iter().find(|c| c.id() == id)
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let instructions: Vec<Span> = vec![
            " ".into(),
            span!("q").green().bold(),
            span!(" to close, ").white(),
            span!("Enter").green().bold(),
            span!(" to select, ").white(),
            span!("d").green().bold(),
            span!(" to delete, ").white(),
            span!("r").green().bold(),
            span!(" to rename ").white(),
            span!("/").green().bold(),
            span!(" to search ").white(),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightBlue))
            .padding(Padding::new(1, 1, 0, 0))
            .title(Line::from(" Chat History ").bold())
            .title_alignment(Alignment::Center)
            .title_bottom(Line::from(instructions))
            .style(Style::default());

        let inner = block.inner(area);
        if !self.showing {
            if !self.conversations.is_empty() && self.items.is_empty() {
                self.last_known_width = (inner.width - 2) as usize;
                self.update_items();
            }
            return;
        }

        f.render_widget(Clear, area);

        if self.last_known_width != (inner.width - 2) as usize {
            self.last_known_width = (inner.width - 2) as usize;
            self.update_items();
        }

        let list = List::new(self.items.clone())
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        f.render_stateful_widget(list, inner, &mut self.state);

        let rename_area = input_box::build_area(inner, ((inner.width as f32 * 0.8).ceil()) as u16);
        self.rename.render(f, rename_area);

        self.question.render(f, inner);
        let search_area = input_box::build_area(inner, ((inner.width as f32 * 0.8).ceil()) as u16);
        self.search.render(f, search_area);
    }
}

impl Display for ConversationGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConversationGroup::Today => write!(f, "Today"),
            ConversationGroup::Yesterday => write!(f, "Yesterday"),
            ConversationGroup::Last7Days => write!(f, "Last 7 Days"),
            ConversationGroup::Last30Days => write!(f, "Last 30 Days"),
            ConversationGroup::Older => write!(f, "Older"),
        }
    }
}

impl ConversationGroup {
    fn to_text<'b>(&self) -> Text<'b> {
        Text::from(self.to_string())
            .alignment(Alignment::Center)
            .bold()
    }
    fn to_list_item<'b>(&self) -> ListItem<'b> {
        ListItem::new(self.to_text())
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Rgb(26, 35, 126)),
            )
            .add_modifier(Modifier::BOLD)
    }
}

fn categorize_conversation(
    now: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
) -> ConversationGroup {
    let age =
        now.with_timezone(&Local).date_naive() - updated_at.with_timezone(&Local).date_naive();
    let days = age.num_days();
    match days {
        0 => ConversationGroup::Today,
        1 => ConversationGroup::Yesterday,
        2..=7 => ConversationGroup::Last7Days,
        8..=30 => ConversationGroup::Last30Days,
        _ => ConversationGroup::Older,
    }
}
