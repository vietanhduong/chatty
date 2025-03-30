use std::time::Duration;
use std::{collections::HashMap, io, sync::Arc, time};

use crate::config::Configuration;
use crate::context::Compressor;
use crate::models::action::Action;
use crate::models::conversation::FindMessage;
use crate::models::{BackendPrompt, Conversation, Event, Message, message::Issuer};
use crate::models::{BackendResponse, Model, UpsertConvoRequest};
use crate::{info_notice, warn_notice};
use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use eyre::Result;
use ratatui::crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    prelude::{Backend, CrosstermBackend},
    style::Stylize,
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation},
};
use ratatui_macros::span;
use syntect::highlighting::Theme;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::{
    app::app_state::AppState,
    app::ui::{
        EditScreen, HelpScreen, HistoryScreen, Loading, ModelsScreen, Notice, TextArea, utils,
    },
};

use super::services::EventService;

const MIN_WIDTH: u16 = 80;

pub struct InitProps {
    pub models: Vec<Model>,
    pub conversations: HashMap<String, Conversation>,
}

pub struct App<'a> {
    action_tx: mpsc::UnboundedSender<Action>,
    event_tx: mpsc::UnboundedSender<Event>,

    events: &'a mut EventService,

    app_state: AppState<'a>,
    models_screen: ModelsScreen<'a>,
    help_screen: HelpScreen<'a>,
    edit_screen: EditScreen<'a>,
    history_screen: HistoryScreen<'a>,
    input: tui_textarea::TextArea<'a>,

    compressor: Arc<Compressor>,

    notice: Notice,
    loading: Loading<'a>,

    cancel_token: CancellationToken,
}

impl<'a> App<'a> {
    pub fn new(
        theme: Theme,
        action_tx: mpsc::UnboundedSender<Action>,
        events: &'a mut EventService,
        compressor: Arc<Compressor>,
        cancel_token: CancellationToken,

        init_props: InitProps,
    ) -> App<'a> {
        let theme = Box::leak(Box::new(theme));
        let mut conversations = init_props.conversations;
        conversations.insert(String::new(), Conversation::new_hello());

        let event_tx = events.event_tx();
        App {
            action_tx: action_tx.clone(),
            event_tx: event_tx.clone(),
            compressor,
            edit_screen: EditScreen::new(theme, action_tx.clone()),
            events,
            app_state: AppState::new(theme),
            input: TextArea::default().build(),
            loading: Loading::new(vec![
                span!("Thinking... Press ").gray(),
                span!("Ctrl+c").green().bold(),
                span!(" to abort!").gray(),
            ]),
            help_screen: HelpScreen::new(),
            history_screen: HistoryScreen::new(action_tx)
                .with_conversations(conversations)
                .with_current_conversation(""),
            models_screen: ModelsScreen::new(init_props.models, event_tx.clone()),
            notice: Notice::default(),
            cancel_token,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let stdout = io::stdout();
        let mut stdout = stdout.lock();

        enable_raw_mode()?;
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;

        let term_backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(term_backend)?;
        let result = self.start_loop(&mut terminal).await;

        self.cancel_token.cancel();

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste
        )?;

        terminal.show_cursor()?;
        result
    }

    async fn handle_key_event(&mut self) -> bool {
        let event = self.events.next().await;

        // Handle critical events first
        if let Some(stop) = self.handle_global_event(&event).await {
            return stop;
        }

        // Handle screen events
        if self.help_screen.showing() {
            if self.help_screen.handle_key_event(&event) {
                self.event_tx.send(Event::Quit).ok();
            }
            return false;
        }

        if self.models_screen.showing() {
            if self.models_screen.handle_key_event(&event).await {
                self.event_tx.send(Event::Quit).ok();
            }
            return false;
        }

        if self.edit_screen.showing() {
            if self.edit_screen.handle_key_event(&event).await {
                self.event_tx.send(Event::Quit).ok();
            }
            return false;
        }

        if self.history_screen.showing() {
            if self.history_screen.handle_key_event(&event).await {
                self.event_tx.send(Event::Quit).ok();
            }
            return false;
        }

        self.handle_input_event(event).await;
        false
    }

    async fn handle_global_event(&mut self, event: &Event) -> Option<bool> {
        match &event {
            Event::Quit => {
                self.save_last_message();

                if self.app_state.waiting_for_backend {
                    self.app_state.waiting_for_backend = false;
                    self.handle_abort();
                }

                sleep(time::Duration::from_millis(100)).await;
                return Some(true);
            }

            Event::BackendAbort => {
                self.handle_abort();
                return Some(false);
            }

            Event::BackendPromptResponse(resp) => {
                self.handle_response(resp);
                return Some(false);
            }

            Event::BackendMessage(msg) => {
                self.app_state.add_message(msg.clone());
                let convo_id = self.app_state.current_convo.id();
                let _ = self
                    .action_tx
                    .send(Action::UpsertMessage(convo_id.to_string(), msg.clone()));
                self.app_state.waiting_for_backend = false;
                Some(false)
            }

            Event::ConversationDeleted(id) => {
                self.history_screen.remove_conversation(&id);
                if self.app_state.current_convo.id() == id {
                    self.upsert_default_conversation();
                    self.app_state.set_conversation(Conversation::new_hello());
                    self.change_conversation(Conversation::new_hello(), false);
                }
                Some(false)
            }

            Event::ConversationUpdated(convo) => {
                let mut convo = convo.clone();

                if let Some(last) = self.app_state.current_convo.last_message() {
                    convo.set_updated_at(last.created_at());
                }
                if self.app_state.current_convo.id() == convo.id() {
                    self.app_state.set_conversation(convo);
                }
                Some(false)
            }

            Event::SetConversation(convo) => {
                self.change_conversation(convo.clone().unwrap_or(Conversation::new_hello()), false);
                Some(false)
            }

            Event::Notice(msg) => {
                self.notice.add_message(msg.clone());
                Some(false)
            }

            // Fallthrough to the next event handler
            _ => None,
        }
    }

    async fn handle_input_event(&mut self, event: Event) {
        // Handle input events
        match event {
            Event::KeyboardCharInput(c) => {
                if !self.app_state.waiting_for_backend {
                    self.input.input(c);
                }
            }

            Event::KeyboardCtrlC => {
                if self.app_state.waiting_for_backend {
                    self.app_state.waiting_for_backend = false;
                    let _ = self.action_tx.send(Action::BackendAbort);
                    return;
                }

                // Clear text in the input area if not waiting for backend
                if !self.input.lines().is_empty() {
                    self.input = TextArea::default().build();
                }
            }

            Event::KeyboardF1 => self.help_screen.toggle_showing(),

            Event::KeyboardCtrlN => self.handle_new_conversation(),

            Event::KeyboardCtrlH => {
                if !self.on_waiting_backend(true) {
                    self.history_screen.toggle_showing();
                }
            }

            Event::KeyboardCtrlL => self.models_screen.toggle_showing(),

            Event::KeyboardCtrlE => {
                if !self.on_waiting_backend(true) {
                    self.edit_screen
                        .set_messages(self.app_state.current_convo.messages());
                    self.edit_screen.toggle_showing();
                }
            }

            Event::KeyboardCtrlR => self.handle_regenerate_response().await,

            Event::KeyboardPaste(text) => {
                self.input.set_yank_text(text.replace('\r', "\n"));
                self.input.paste();
            }

            Event::KeyboardAltEnter => {
                if !self.on_waiting_backend(false) {
                    self.input.insert_newline();
                }
            }

            Event::KeyboardEnter => self.handle_send_prompt(),

            Event::UiScrollDown => self.app_state.scroll.down(),
            Event::UiScrollUp => self.app_state.scroll.up(),
            Event::UiScrollPageDown => self.app_state.scroll.page_down(),
            Event::UiScrollPageUp => self.app_state.scroll.page_up(),
            _ => {}
        }
    }

    fn render<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        terminal.draw(|f| {
            let current_width = f.area().width;
            if !is_line_width_sufficient(current_width) {
                f.render_widget(
                    Paragraph::new(utils::split_to_lines(
                        format!(
                            "I'm too small, make me bigger! I need at least {} cells (current: {})",
                            MIN_WIDTH, current_width
                        ),
                        (current_width - 2) as usize,
                    ))
                    .alignment(Alignment::Left),
                    f.area(),
                );
                return;
            }

            let textarea_len = (self.input.lines().len() + 2).try_into().unwrap();
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![
                    Constraint::Min(1),
                    Constraint::Max(textarea_len),
                    Constraint::Length(1),
                ])
                .split(f.area());

            if layout[0].width as usize != self.app_state.last_known_width
                || layout[0].height as usize != self.app_state.last_known_height
            {
                self.app_state.set_rect(layout[0]);
            }

            self.app_state.bubble_list.render(
                layout[0],
                f.buffer_mut(),
                self.app_state.scroll.position.try_into().unwrap(),
            );

            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .end_symbol(None)
                    .begin_symbol(None),
                layout[0].inner(Margin {
                    vertical: 1,
                    horizontal: 1,
                }),
                &mut self.app_state.scroll.scrollbar_state,
            );

            self.help_screen.render_help_line(f, layout[2]);
            if self.app_state.waiting_for_backend {
                self.loading.render(f, layout[1]);
            } else {
                f.render_widget(&self.input, layout[1]);
            }

            self.help_screen
                .render(f, utils::popup_area(f.area(), 40, 30));

            self.models_screen
                .render(f, utils::popup_area(f.area(), 40, 60));

            self.edit_screen
                .render(f, utils::popup_area(f.area(), 70, 90));
            self.history_screen
                .render(f, utils::popup_area(f.area(), 70, 90));

            self.notice.render(f, utils::notice_area(f.area(), 30));
        })?;
        Ok(())
    }

    async fn start_loop<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            self.render(terminal)?;
            if self.handle_key_event().await {
                return Ok(());
            }
        }
    }

    fn handle_send_prompt(&mut self) {
        if self.on_waiting_backend(false) {
            return;
        }

        let input_str = &self.input.lines().join("\n");
        if input_str.is_empty() {
            return;
        }

        let first = self.app_state.current_convo.len() < 2;

        let msg = Message::new_user("user", input_str);
        self.input = TextArea::default().build();
        self.app_state.add_message(msg.clone());

        if self.app_state.current_convo.id().is_empty() {
            // Default conversation
            let conversation_id = uuid::Uuid::new_v4().to_string();
            self.app_state.current_convo.set_id(&conversation_id);

            self.history_screen.remove_conversation("");
            self.history_screen
                .add_conversation_and_set(&self.app_state.current_convo);
        }

        self.app_state.waiting_for_backend = true;

        let convo_id = self.app_state.current_convo.id().to_string();
        let model = self.models_screen.current_model();

        let prompt = BackendPrompt::new(input_str)
            .with_context(self.app_state.current_convo.build_context())
            .with_model(model);

        if first {
            self.save_current_conversation(true);

            // Save the first message to the storage
            let _ = self.action_tx.send(Action::UpsertMessage(
                convo_id.to_string(),
                self.app_state.current_convo.messages()[0].clone(),
            ));
        }

        // Save the current message to the storage
        let _ = self
            .action_tx
            .send(Action::UpsertMessage(convo_id.to_string(), msg.clone()));

        self.history_screen
            .update_conversation_updated_at(&convo_id, msg.created_at());

        let _ = self.action_tx.send(Action::BackendRequest(prompt));
        self.history_screen.update_items();
    }

    async fn handle_regenerate_response(&mut self) {
        if self.on_waiting_backend(true) {
            return;
        }

        // Rebuild the conversation by removing all the messages from the backend
        // and resubmit the last message from user
        {
            let mut i = self.app_state.current_convo.len() as i32 - 1;
            if i == 0 {
                // Welcome message, nothing to do
                return;
            }

            while i >= 0 {
                if !self.app_state.current_convo.messages_mut()[i as usize].is_system() {
                    break;
                }
                let msg = self
                    .app_state
                    .current_convo
                    .messages_mut()
                    .remove(i as usize);
                self.app_state
                    .bubble_list
                    .remove_message_by_index(i as usize);
                // Tell the storage to remove the message
                let _ = self
                    .action_tx
                    .send(Action::DeleteMessage(msg.id().to_string()));

                i -= 1;
            }
        }
        self.app_state.sync_state();
        self.app_state.scroll.last();

        // Resubmit the last message from user
        let last_user_msg = self
            .app_state
            .current_convo
            .last_message_of(Some(Issuer::user()));

        let input_str = if let Some(msg) = last_user_msg {
            msg.text().to_string()
        } else {
            return; // This should never happen
        };

        let model = self.models_screen.current_model();
        self.app_state.waiting_for_backend = true;
        let prompt = BackendPrompt::new(input_str)
            .with_model(model)
            .with_context(self.app_state.current_convo.build_context());

        let _ = self.action_tx.send(Action::BackendRequest(prompt));
    }

    fn handle_abort(&mut self) {
        if let Some(msg) = self.app_state.current_convo.last_message() {
            let convo_id = self.app_state.current_convo.id().to_string();
            let _ = self
                .action_tx
                .send(Action::UpsertMessage(convo_id, msg.clone()));
        }

        let message = Message::new_system("system", "Aborted!");
        self.app_state.add_message(message.clone());
    }

    fn handle_response(&mut self, resp: &BackendResponse) {
        let notify = resp.done && resp.init_conversation;
        let done = resp.done;
        self.app_state.handle_backend_response(&resp);

        if !done {
            return;
        }

        if let Some(ref usage) = resp.usage {
            let convo_id = self.app_state.current_convo.id().to_string();
            if let Some(msg) = self
                .app_state
                .current_convo
                .last_message_of_mut(Some(Issuer::user()))
            {
                msg.set_token_count(usage.prompt_tokens);
                let _ = self
                    .action_tx
                    .send(Action::UpsertMessage(convo_id.to_string(), msg.clone()));
            }

            if let Some(msg) = self
                .app_state
                .current_convo
                .last_message_of_mut(Some(Issuer::system()))
            {
                msg.set_token_count(usage.completion_tokens);
            }

            if Configuration::instance()
                .general
                .show_usage
                .unwrap_or_default()
            {
                self.notice.add_message(info_notice!(
                    format!("Usage: {}", usage.to_string()),
                    Duration::from_secs(7)
                ));
            }
        }

        if notify {
            let title = self.app_state.current_convo.title();
            self.notice.add_message(info_notice!(
                format!("Update conversation's title to \"{}\"", title),
                Duration::from_secs(5)
            ));
            // This will update the conversation title in the history
            self.history_screen
                .upsert_conversation(&self.app_state.current_convo);
        }

        // Update the conversation updated_at in the history
        self.history_screen.update_conversation_updated_at(
            &self.app_state.current_convo.id(),
            self.app_state.current_convo.updated_at(),
        );

        let convo_id = self.app_state.current_convo.id().to_string();
        let last_message = self.app_state.current_convo.last_message().unwrap().clone();
        // Upsert message to the storage
        let _ = self
            .action_tx
            .send(Action::UpsertMessage(convo_id, last_message));

        // If the conversation should be compressed, we will process it
        // in the background and notify the app when it's done to fetch
        // the context and update the conversation. This will mitigate
        // impact to the current conversation.
        if self
            .compressor
            .should_compress(&self.app_state.current_convo)
        {
            let convo_id = self.app_state.current_convo.id().to_string();
            let model = self.models_screen.current_model().to_string();
            let _ = self
                .action_tx
                .send(Action::CompressConversation(convo_id, model));
        }
    }

    fn handle_new_conversation(&mut self) {
        if self.on_waiting_backend(true) {
            return;
        }

        if self.app_state.current_convo.len() < 2 {
            return;
        }
        self.upsert_default_conversation();
        self.change_conversation(Conversation::new_hello(), false);
    }

    fn upsert_default_conversation(&mut self) {
        let convo = Conversation::new_hello();
        self.history_screen.add_conversation_and_set(&convo);
    }

    fn save_last_message(&mut self) {
        self.save_current_conversation(false);
        // If no message is waiting, we can simply return the process
        if !self.app_state.waiting_for_backend {
            return;
        }

        // Otherwise, let save the last message in the conversation
        let convo_id = self.app_state.current_convo.id();
        if let Some(last) = self.app_state.current_convo.last_message() {
            let _ = self
                .action_tx
                .send(Action::UpsertMessage(convo_id.to_string(), last.clone()));
        }
    }

    fn save_current_conversation(&mut self, save_messages: bool) {
        if self.app_state.current_convo.len() < 2 {
            return;
        }

        let _ = self
            .action_tx
            .send(Action::UpsertConversation(UpsertConvoRequest {
                convo: self.app_state.current_convo.clone(),
                include_context: false,
                include_messages: save_messages,
            }));
    }

    fn change_conversation(&mut self, convo: Conversation, save_messages: bool) {
        // Save the current conversation
        self.save_current_conversation(save_messages);

        // Change the conversation
        self.history_screen.set_current_conversation(convo.id());
        let title = convo.title().to_string();
        self.app_state.set_conversation(convo);
        self.notice.info(format!("Switching to \"{}\"", title));
        self.input = TextArea::default().build();
        self.app_state.sync_state();
    }

    fn on_waiting_backend(&mut self, notice: bool) -> bool {
        if self.app_state.waiting_for_backend && notice {
            self.notice.add_message(warn_notice!(
                "Waiting for backend to respond, please wait..."
            ));
        }
        return self.app_state.waiting_for_backend;
    }
}

fn is_line_width_sufficient(line_width: u16) -> bool {
    return line_width >= MIN_WIDTH;
}
