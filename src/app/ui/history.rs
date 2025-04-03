use crate::models::{Action, Conversation, Event, UpsertConvoRequest};
use chrono::{Local, Utc};
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

    action_tx: mpsc::UnboundedSender<Action>,

    conversations: HashMap<String, Conversation>,
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
    pub fn new(action_tx: mpsc::UnboundedSender<Action>) -> HistoryScreen<'a> {
        HistoryScreen {
            action_tx,

            showing: false,
            conversations: HashMap::new(),

            idx_map: HashMap::new(),
            rename: InputBox::default().with_title(" Rename "),
            search: InputBox::default().with_title(" Search "),
            question: Question::default().with_title(" Delete Conversation "),

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

    pub fn with_conversations(
        mut self,
        conversations: HashMap<String, Conversation>,
    ) -> HistoryScreen<'a> {
        self.conversations = conversations;
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
        if let Some(convo) = self.conversations.remove(conversation) {
            if convo.id() == self.current_conversation.as_deref().unwrap_or_default() {
                self.current_conversation = None;
            }
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
        self.conversations.insert(
            conversation.id().to_string(),
            Conversation::default()
                .with_title(conversation.title())
                .with_id(conversation.id())
                .with_created_at(conversation.created_at())
                .with_updated_at(conversation.updated_at()),
        );
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

    pub fn update_conversation_updated_at(
        &mut self,
        conversation_id: &str,
        updated_at: chrono::DateTime<Utc>,
    ) {
        if let Some(conversation) = self.conversations.get_mut(conversation_id) {
            conversation.set_updated_at(updated_at);
            self.update_items();
        }
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
            .filter(|(_, c)| {
                if self.current_search.is_empty() {
                    return true;
                }
                c.title()
                    .to_lowercase()
                    .contains(&self.current_search.to_lowercase())
            })
            .for_each(|(_, c)| {
                let group = categorize_conversation(now, c.updated_at());

                conversations.entry(group).or_default().push(c);
            });

        for (group, mut conversations) in conversations {
            self.items.push(group.to_list_item());
            conversations.sort_by(|a, b| {
                // If the id is empty, always put it at the top of the list
                if a.id().is_empty() {
                    return std::cmp::Ordering::Less;
                }

                b.updated_at()
                    .with_timezone(&Local)
                    .cmp(&a.updated_at().with_timezone(&Local))
            });

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

    pub async fn handle_key_event(&mut self, event: &Event) -> bool {
        if self.rename.showing() {
            self.handle_rename_popup(event).await;
            return false;
        }

        if self.question.showing() {
            self.handle_question_popup(event).await;
            return false;
        }

        if self.search.showing() {
            self.handle_search_popup(event).await;
            return false;
        }

        match event {
            Event::KeyboardCtrlH => {
                self.showing = !self.showing;
            }
            Event::Quit => {
                self.showing = false;
                return true;
            }

            Event::KeyboardEnter => {
                if self.state.selected().is_none() || self.conversations.is_empty() {
                    return false;
                }

                let id = match self.get_selected_conversation_id() {
                    Some(id) => id.to_string(),
                    None => return false,
                };

                if self.current_conversation.as_deref() == Some(&id) {
                    return false;
                }

                self.showing = false;
                self.action_tx
                    .send(Action::SetConversation(id.clone()))
                    .ok();
                self.current_conversation = Some(id);
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
                        None => return false,
                    };

                    if conversation.id().is_empty() {
                        return false;
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
                            return false;
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
        false
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
        if let Event::KeyboardCharInput(input) = event {
            match input.key {
                Key::Char('y') => {
                    self.on_delete().await;
                    self.question.close();
                }
                Key::Char('n') | Key::Char('q') => {
                    self.question.close();
                }
                _ => {}
            }
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

        log::debug!("Deleting conversation: {}", convo_id);

        let _ = self
            .action_tx
            .send(Action::DeleteConversation(convo_id.to_string()));
    }

    pub async fn rename_conversation(&mut self, new_title: &str) {
        let convo_id = match self.get_selected_conversation_id() {
            Some(id) => id.to_string(),
            None => return,
        };

        let convo = match self.conversations.get_mut(&convo_id) {
            Some(c) => c,
            None => return,
        };

        log::debug!(
            "Renaming conversation: {}, old/new: {}/{}",
            convo.id(),
            convo.title(),
            new_title
        );

        if convo.title() == new_title || new_title.is_empty() {
            return;
        }

        convo.set_title(new_title.to_string());
        let convo = convo.clone();
        self.update_items();

        let _ = self
            .action_tx
            .send(Action::UpsertConversation(UpsertConvoRequest {
                convo,
                include_context: false,
                include_messages: false,
            }));
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
        self.conversations.get(&id)
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
