use chrono::{Local, Utc};
use eyre::Result;
use openai_models::{Conversation, Event, NoticeMessage};
use openai_storage::ArcStorage;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding},
};
use ratatui_macros::span;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fmt::Display,
    rc::Rc,
};
use tokio::sync::mpsc;
use tui_textarea::Key;

use super::{
    helpers,
    question::Question,
    rename::{self, Rename},
};

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
    conversations: Vec<Rc<RefCell<Conversation>>>,
    list_items: Vec<ListItem<'a>>,
    id_map: HashMap<usize, String>,

    rename: Rename<'a>,
    question: Question<'a>,

    current_conversation: Option<String>,
    list_state: ListState,
}

impl<'a> HistoryScreen<'a> {
    pub fn new(event_tx: mpsc::UnboundedSender<Event>, storage: ArcStorage) -> HistoryScreen<'a> {
        HistoryScreen {
            event_tx,
            storage,
            showing: false,
            conversations: vec![],
            list_items: vec![],
            id_map: HashMap::new(),
            rename: Rename::default(),
            current_conversation: None,
            list_state: ListState::default(),
            question: Question::new().with_title(" Delete Conversation "),
        }
    }

    pub fn showing(&self) -> bool {
        self.showing
    }

    pub fn with_conversations(
        mut self,
        conversations: Vec<Rc<RefCell<Conversation>>>,
    ) -> HistoryScreen<'a> {
        self.conversations = conversations;
        // sort the conversations by last updated time descending
        self.conversations
            .sort_by(|a, b| b.borrow().updated_at().cmp(&a.borrow().updated_at()));
        self.conversations
            .dedup_by(|a, b| a.borrow().id() == b.borrow().id());
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
            .position(|c| c.borrow().id() == conversation)
        {
            if self.current_conversation.as_deref() == Some(conversation) {
                self.current_conversation = None;
            }
            self.conversations.remove(pos);
        }
    }

    fn move_cursor_to_current(&mut self) {
        if let Some(current_conversation) = self.current_conversation.as_ref() {
            let pos = self
                .id_map
                .iter()
                .find(|(_, id)| *id == current_conversation)
                .map(|(pos, _)| *pos);
            self.list_state.select(pos);
        }
    }

    pub fn add_conversation(&mut self, conversation: Rc<RefCell<Conversation>>) {
        // If the conversation already exists, just update it
        // otherwise, add the conversation at the top of the list
        let pos = self
            .conversations
            .iter()
            .position(|c| c.borrow().id() == conversation.borrow().id())
            .unwrap_or_default();

        if pos != 0 {
            log::debug!(
                "Conversation already exists, updating it: {:?}",
                conversation
            );
            // remove the conversation from the list
            self.conversations.remove(pos);
        }

        self.current_conversation = Some(conversation.borrow().id().to_string());
        self.conversations.insert(0, conversation);
        // sort the conversations by last updated time descending
        self.conversations
            .sort_by(|a, b| b.borrow().updated_at().cmp(&a.borrow().updated_at()));
    }

    pub fn current_conversation(&self) -> Option<String> {
        self.current_conversation.clone()
    }

    pub fn set_current_conversation(&mut self, conversation: impl Into<String>) {
        self.current_conversation = Some(conversation.into());
        self.move_cursor_to_current();
    }

    fn next_row(&mut self) {
        if self.conversations.is_empty() {
            self.list_state.select(None);
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => (i + 1).min(self.list_items.len() - 1),
            None => 0,
        };
        // If i is not present in the index map, which means it is a group header, we need to
        // find the next item that is not a group header
        if self.id_map.get(&i).is_none() {
            let mut next = i + 1;
            while next < self.list_items.len() && self.id_map.get(&next).is_none() {
                next += 1;
            }
            if next < self.list_items.len() {
                self.list_state.select(Some(next));
            }
            // Do nothing if next is out of bounds
            return;
        }
        self.list_state.select(Some(i));
    }

    fn prev_row(&mut self) {
        if self.conversations.is_empty() {
            self.list_state.select(None);
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => (i as isize - 1).max(0) as usize,
            None => 0,
        };

        // If i is not present in the index map, which means it is a group header, we need to
        // find the previous item that is not a group header
        if self.id_map.get(&i).is_none() {
            let mut prev = i as isize - 1;
            while prev >= 0 && self.id_map.get(&(prev as usize)).is_none() {
                prev -= 1;
            }
            if prev >= 0 {
                self.list_state.select(Some(prev as usize));
            }
            return;
        }
        self.list_state.select(Some(i));
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

    fn build_list_items(&mut self, max_width: usize) {
        self.list_items.clear();
        self.id_map.clear();

        if self.conversations.is_empty() {
            self.list_items.push(ListItem::new(
                Text::from(NO_CONVERSATIONS).alignment(Alignment::Center),
            ));
            self.list_state.select(None);
            return;
        }

        let mut conversations: BTreeMap<ConversationGroup, Vec<Rc<RefCell<Conversation>>>> =
            BTreeMap::new();

        let now = Utc::now();
        for conversation in &self.conversations {
            let group = categorize_conversation(now, conversation);

            conversations
                .entry(group)
                .or_insert_with(Vec::new)
                .push(conversation.clone());
        }

        for (group, conversations) in conversations {
            self.list_items.push(group.to_list_item());

            for c in conversations {
                let mut spans = c
                    .borrow()
                    .title()
                    .split(' ')
                    .map(|s| Span::raw(s.to_string()))
                    .collect::<Vec<_>>();
                if self.current_conversation.as_deref() == Some(c.borrow().id()) {
                    spans.push(Span::styled("[*]", Style::default().fg(Color::LightRed)))
                }

                let lines = helpers::split_to_lines(spans, max_width);
                self.list_items.push(ListItem::new(Text::from(lines)));
                self.id_map
                    .insert(self.list_items.len() - 1, c.borrow().id().to_string());
            }
        }
    }

    pub async fn handle_key_event(&mut self, event: &Event) -> Result<bool> {
        if self.rename.showing() {
            match event {
                Event::KeyboardEnter => {
                    let text = self.rename.close().unwrap_or_default();
                    self.on_rename(text).await
                }
                Event::KeyboardCtrlC | Event::KeyboardEsc => {
                    self.rename.close();
                }
                _ => self.rename.handle_key_event(event),
            }
            return Ok(false);
        }

        if self.question.showing() {
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
                if self.list_state.selected().is_none() || self.conversations.is_empty() {
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
                Key::Char('q') => {
                    self.showing = false;
                    return Ok(false);
                }
                Key::Char('d') => {
                    let conversation = match self.get_selected_conversation() {
                        Some(c) => c,
                        None => return Ok(false),
                    };

                    if conversation.borrow().len() < 2 {
                        return Ok(false);
                    }

                    let quest = vec![
                        span!("Do you want to delete"),
                        span!(format!("\"{}\"", conversation.borrow().title()))
                            .add_modifier(Modifier::BOLD | Modifier::ITALIC)
                            .yellow(),
                        span!("?"),
                    ];
                    self.question.open(quest);
                }
                Key::Char('r') => {
                    if let Some(conversation) = self.get_selected_conversation() {
                        // Ignore with blank conversation
                        if conversation.borrow().len() < 2 {
                            return Ok(false);
                        }
                        let title = conversation.borrow().title().to_string();
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

    async fn on_delete(&mut self) {
        let conversation = match self.get_selected_conversation() {
            Some(c) => c,
            None => return,
        };

        let convo_id = conversation.borrow().id().to_string();
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

    async fn on_rename(&mut self, new_title: String) {
        let conversation = match self.get_selected_conversation() {
            Some(c) => c,
            None => return,
        };

        if new_title.is_empty() || new_title == conversation.borrow().title() {
            return;
        }

        conversation.borrow_mut().set_title(new_title.clone());
        let conversation = conversation.borrow().clone();
        if let Err(err) = self.storage.upsert_conversation(conversation).await {
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
        if self.list_state.selected().is_none() || self.conversations.is_empty() {
            return None;
        }
        let idx = self.list_state.selected().unwrap();

        match self.id_map.get(&idx) {
            Some(id) => Some(id),
            _ => None,
        }
    }

    pub fn get_selected_conversation(&self) -> Option<Rc<RefCell<Conversation>>> {
        let id = self.get_selected_conversation_id()?;
        self.conversations
            .iter()
            .find(|c| c.borrow().id() == id)
            .cloned()
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if !self.showing {
            if !self.conversations.is_empty() && self.list_items.is_empty() {
                self.build_list_items((area.width - 2) as usize);
            }
            return;
        }

        let instructions: Vec<Span> = vec![
            " ".into(),
            span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "q"),
            " to close, ".into(),
            span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "Enter"),
            " to select, ".into(),
            span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "d"),
            " to delete, ".into(),
            span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "r"),
            " to rename, ".into(),
            span!(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD); "↑/k/↓/j"),
            " to move up/down ".into(),
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

        f.render_widget(Clear, area);
        let inner = block.inner(area);
        self.build_list_items((inner.width - 2) as usize);

        let list = List::new(self.list_items.clone())
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        f.render_stateful_widget(list, inner, &mut self.list_state);

        let rename_area = rename::rename_area(inner, ((inner.width as f32 * 0.8).ceil()) as u16);
        self.rename.render(f, rename_area);

        self.question.render(f, inner);
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
            .style(Style::default().fg(Color::White).bg(Color::LightBlue))
            .add_modifier(Modifier::BOLD)
    }
}

fn categorize_conversation(
    now: chrono::DateTime<Utc>,
    conversation: &Rc<RefCell<Conversation>>,
) -> ConversationGroup {
    let age = now.with_timezone(&Local).date_naive()
        - conversation
            .borrow()
            .updated_at()
            .with_timezone(&Local)
            .date_naive();
    let days = age.num_days();
    match days {
        0 => ConversationGroup::Today,
        1 => ConversationGroup::Yesterday,
        2..=7 => ConversationGroup::Last7Days,
        8..=30 => ConversationGroup::Last30Days,
        _ => ConversationGroup::Older,
    }
}
