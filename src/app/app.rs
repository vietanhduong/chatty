use std::{collections::HashMap, io, sync::Arc, time};

use crate::config::Configuration;
use crate::context::Compressor;
use crate::models::conversation::FindMessage;
use crate::models::{
    Action, BackendPrompt, Conversation, Event, Message, NoticeMessage, NoticeType, message::Issuer,
};
use crate::models::{BackendResponse, Model};
use crate::storage::ArcStorage;
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

use crate::{
    app::app_state::AppState,
    app::services::EventsService,
    app::ui::{
        EditScreen, HelpScreen, HistoryScreen, Loading, ModelsScreen, Notice, TextArea, utils,
    },
};

const MIN_WIDTH: u16 = 80;

pub struct AppInitProps {
    pub models: Vec<Model>,
    pub default_model: String,
    pub conversations: HashMap<String, Conversation>,
}

pub struct App<'a> {
    action_tx: mpsc::UnboundedSender<Action>,
    event_tx: mpsc::UnboundedSender<Event>,

    events: EventsService<'a>,

    app_state: AppState<'a>,
    models_screen: ModelsScreen<'a>,
    help_screen: HelpScreen<'a>,
    edit_screen: EditScreen<'a>,
    history_screen: HistoryScreen<'a>,
    input: tui_textarea::TextArea<'a>,

    compressor: Arc<Compressor>,
    storage: ArcStorage,

    notice: Notice,
    loading: Loading<'a>,
}

impl<'a> App<'a> {
    pub fn new(
        theme: &'a Theme,
        action_tx: mpsc::UnboundedSender<Action>,
        event_tx: mpsc::UnboundedSender<Event>,
        event_rx: &'a mut mpsc::UnboundedReceiver<Event>,
        compressor: Arc<Compressor>,
        storage: ArcStorage,
        init_props: AppInitProps,
    ) -> App<'a> {
        let mut conversations = init_props
            .conversations
            .into_iter()
            .map(|(id, convo)| {
                let convo = Conversation::default()
                    .with_title(convo.title())
                    .with_id(&id)
                    .with_created_at(convo.created_at())
                    .with_updated_at(convo.updated_at());
                (id, convo)
            })
            .collect::<HashMap<_, _>>();

        conversations.insert(String::new(), Conversation::new_hello());

        App {
            event_tx: event_tx.clone(),
            compressor,
            storage: storage.clone(),
            edit_screen: EditScreen::new(action_tx.clone(), theme),
            action_tx: action_tx.clone(),
            events: EventsService::new(event_rx),
            app_state: AppState::new(theme),
            input: TextArea::default().build(),
            loading: Loading::new(vec![
                span!("Thinking... Press ").gray(),
                span!("Ctrl+c").green().bold(),
                span!(" to abort!").gray(),
            ]),
            help_screen: HelpScreen::new(),
            history_screen: HistoryScreen::new(event_tx.clone(), storage)
                .with_conversations(conversations)
                .with_current_conversation(""),
            models_screen: ModelsScreen::new(
                init_props.default_model,
                init_props.models,
                action_tx.clone(),
            ),
            notice: Notice::default(),
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
        self.start_loop(&mut terminal).await?;

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste
        )?;

        terminal.show_cursor()?;

        Ok(())
    }

    async fn handle_key_event(&mut self) -> Result<bool> {
        let event = self.events.next().await?;

        // Handle critical events first
        match &event {
            Event::Quit => {
                self.save_last_message().await?;
                if self.app_state.waiting_for_backend {
                    self.app_state.waiting_for_backend = false;
                    self.action_tx.send(Action::BackendAbort)?;
                }
                return Ok(true);
            }
            Event::BackendPromptResponse(resp) => {
                self.handle_response(resp).await;
                return Ok(false);
            }

            Event::BackendMessage(msg) => {
                self.app_state.add_message(msg.clone());
                self.storage
                    .upsert_message(self.app_state.current_convo.id(), msg.clone())
                    .await?;
                self.app_state.waiting_for_backend = false;
            }

            Event::AbortRequest => {
                self.handle_abort().await;
                return Ok(false);
            }

            Event::ConversationDeleted(id) => {
                self.history_screen.remove_conversation(&id);
                if self.app_state.current_convo.id() == id {
                    self.upsert_default_conversation();
                    self.change_conversation("").await;
                }
                return Ok(false);
            }

            Event::ConversationUpdated(id) => {
                if let Some(mut convo) = self.storage.get_conversation(&id).await? {
                    let updated_at = convo.messages().last().unwrap().created_at();
                    convo.set_updated_at(updated_at);
                    if self.app_state.current_convo.id() == id {
                        self.app_state.set_conversation(convo);
                    }
                }
                return Ok(false);
            }

            Event::SetConversation(id) => {
                self.change_conversation(&id).await;
                return Ok(false);
            }

            Event::Notice(msg) => {
                self.notice.add_message(msg.clone());
                return Ok(false);
            }

            Event::ModelChanged(model) => {
                self.models_screen.set_current_model(&model);
                self.notice.info(format!("Using model \"{}\"", model));
                return Ok(false);
            }

            _ => {}
        }

        // Handle screen events
        if self.help_screen.showing() {
            if !self.help_screen.handle_key_event(&event) {
                return Ok(false);
            }
        }

        if self.models_screen.showing() {
            if !self.models_screen.handle_key_event(&event).await? {
                return Ok(false);
            }
        }

        if self.edit_screen.showing() {
            if !self.edit_screen.handle_key_event(&event).await? {
                return Ok(false);
            }
        }

        if self.history_screen.showing() {
            if !self.history_screen.handle_key_event(&event).await? {
                return Ok(false);
            }
        }

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
                    self.action_tx.send(Action::BackendAbort)?;
                    return Ok(false);
                }

                // Clear text in the input area if not waiting for backend
                if !self.input.lines().is_empty() {
                    self.input = TextArea::default().build();
                }
            }

            Event::KeyboardF1 => self.help_screen.toggle_showing(),

            Event::KeyboardCtrlN => self.handle_new_conversation().await,

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

            Event::KeyboardEnter => self.handle_send_prompt().await,

            Event::UiScrollDown => self.app_state.scroll.down(),
            Event::UiScrollUp => self.app_state.scroll.up(),
            Event::UiScrollPageDown => self.app_state.scroll.page_down(),
            Event::UiScrollPageUp => self.app_state.scroll.page_up(),
            _ => {}
        }
        Ok(false)
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
            if self.handle_key_event().await? {
                return Ok(());
            }
        }
    }

    async fn handle_send_prompt(&mut self) {
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

        let conversation_id = self.app_state.current_convo.id().to_string();
        let model = self.models_screen.current_model();

        let prompt = BackendPrompt::new(input_str)
            .with_context(self.app_state.current_convo.build_context())
            .with_model(model);

        if first {
            self.save_current_conversation().await;

            // Save the first message to the storage
            let err = self
                .storage
                .upsert_message(
                    &conversation_id,
                    self.app_state.current_convo.messages()[0].clone(),
                )
                .await;
            if let Err(err) = err {
                self.notice_on_error(format!("Failed to save message: {}", err));
                return;
            }
        }

        // Save the current message to the storage
        if let Err(err) = self
            .storage
            .upsert_message(&conversation_id, msg.clone())
            .await
        {
            self.notice_on_error(format!("Failed to save message: {}", err));
            return;
        }

        self.history_screen
            .update_conversation_updated_at(&conversation_id, msg.created_at());

        if let Err(err) = self.action_tx.send(Action::BackendRequest(prompt)) {
            self.notice_on_error(format!("Failed to send request: {}", err));
            return;
        }

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
                let con = self
                    .app_state
                    .current_convo
                    .messages_mut()
                    .remove(i as usize);
                self.app_state
                    .bubble_list
                    .remove_message_by_index(i as usize);
                // Tell the storage to remove the message
                if let Err(err) = self.storage.delete_messsage(con.id()).await {
                    self.notice_on_error(format!("Failed to delete message: {}", err));
                    return;
                }

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

        if let Err(err) = self.action_tx.send(Action::BackendRequest(prompt)) {
            self.notice_on_error(format!("Failed to send request: {}", err));
        }
    }

    async fn handle_abort(&mut self) {
        if let Some(msg) = self.app_state.current_convo.last_message() {
            if let Err(err) = self
                .storage
                .upsert_message(self.app_state.current_convo.id(), msg.clone())
                .await
            {
                self.notice_on_error(format!("Failed to save message: {}", err));
                return;
            }
        }

        let message = Message::new_system("system", "Aborted!");
        self.app_state.add_message(message.clone());
    }

    async fn handle_response(&mut self, resp: &BackendResponse) {
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
                if let Err(err) = self.storage.upsert_message(&convo_id, msg.clone()).await {
                    self.notice_on_error(format!("Failed to save message: {}", err));
                    return;
                }
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
                self.notice.add_message(
                    NoticeMessage::info(format!("Usage: {}", usage.to_string()))
                        .with_duration(time::Duration::from_secs(6)),
                );
            }
        }

        if notify {
            let title = self.app_state.current_convo.title();
            self.notice.add_message(
                NoticeMessage::new(format!("Updated Title: \"{}\"", title))
                    .with_duration(time::Duration::from_secs(5)),
            );
            // This will update the conversation title in the history
            self.history_screen
                .upsert_conversation(&self.app_state.current_convo);
        }

        // Update the conversation updated_at in the history
        self.history_screen.update_conversation_updated_at(
            &self.app_state.current_convo.id(),
            self.app_state.current_convo.updated_at(),
        );

        // Upsert message to the storage
        let err = self
            .storage
            .upsert_message(
                self.app_state.current_convo.id(),
                self.app_state.current_convo.last_message().unwrap().clone(),
            )
            .await;
        if let Err(err) = err {
            self.notice_on_error(format!("Failed to save message: {}", err));
            return;
        }

        // If the conversation should be compressed, we will process it
        // in the background and notify the app when it's done to fetch
        // the context and update the conversation. This will mitigate
        // impact to the current conversation.
        if self
            .compressor
            .should_compress(&self.app_state.current_convo)
        {
            self.handle_convo_compress(self.app_state.current_convo.id());
        }
    }

    fn handle_convo_compress(&self, conversation_id: &str) {
        let storage = self.storage.clone();
        let compressor = self.compressor.clone();
        let conversation_id = conversation_id.to_string();
        let model = self.models_screen.current_model().to_string();
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            event_tx
                .send(Event::Notice(NoticeMessage::warning(
                    "Compressing conversation... Please do NOT close the app until this process is finished!"
                )))
                .ok();
            let t0 = chrono::Utc::now();
            let conversation = match storage.get_conversation(&conversation_id).await {
                Ok(conversation) => match conversation {
                    Some(conversation) => conversation,
                    None => {
                        log::warn!("Conversation not found");
                        return;
                    }
                },
                Err(err) => {
                    log::error!("Failed to get conversation: {}", err);
                    event_tx
                        .send(Event::Notice(NoticeMessage::warning(format!(
                            "Failed to get conversation: {}",
                            err
                        ))))
                        .ok();
                    return;
                }
            };

            let context = match compressor.compress(&model, &conversation).await {
                Ok(context) => match context {
                    Some(context) => context,
                    None => {
                        log::warn!("No context found");
                        return;
                    }
                },
                Err(err) => {
                    log::error!("Failed to compress conversation: {}", err);
                    event_tx
                        .send(Event::Notice(NoticeMessage::warning(format!(
                            "Failed to compress conversation: {}",
                            err
                        ))))
                        .ok();
                    return;
                }
            };
            // Push the context to the conversation
            match storage.upsert_context(&conversation_id, context).await {
                Ok(_) => {
                    log::info!("Context compressed successfully");
                    event_tx
                        .send(Event::Notice(NoticeMessage::info(format!(
                            "Context compressed successfully in {} seconds",
                            (chrono::Utc::now() - t0).num_seconds()
                        ))))
                        .ok();
                    event_tx
                        .send(Event::ConversationUpdated(conversation_id))
                        .ok();
                }
                Err(err) => {
                    log::error!("Failed to save context: {}", err);
                    event_tx
                        .send(Event::Notice(NoticeMessage::warning(format!(
                            "Failed to save context: {}",
                            err
                        ))))
                        .ok();
                }
            }
        });
    }

    async fn handle_new_conversation(&mut self) {
        if self.on_waiting_backend(true) {
            return;
        }

        if self.app_state.current_convo.len() < 2 {
            return;
        }
        self.upsert_default_conversation();
        self.change_conversation("").await;
    }

    fn upsert_default_conversation(&mut self) {
        let convo = Conversation::new_hello();
        self.history_screen.add_conversation_and_set(&convo);
    }

    async fn save_last_message(&mut self) -> Result<()> {
        self.save_current_conversation().await;
        if !self.app_state.waiting_for_backend {
            // If no message is waiting, we can simply return the process
            return Ok(());
        }
        let conversation_id = self.app_state.current_convo.id();
        // Otherwise, let save the last message in the conversation
        if let Some(last) = self.app_state.current_convo.last_message() {
            self.storage
                .upsert_message(conversation_id, last.clone())
                .await?;
        }
        Ok(())
    }

    async fn save_current_conversation(&mut self) {
        if self.app_state.current_convo.len() < 2 {
            return;
        }
        if let Err(err) = self
            .storage
            .upsert_conversation(self.app_state.current_convo.clone())
            .await
        {
            self.notice.add_message(
                NoticeMessage::new(format!("Failed to save conversation: {}", err))
                    .with_duration(time::Duration::from_secs(5))
                    .with_type(NoticeType::Error),
            );
        }
    }

    async fn change_conversation(&mut self, convo_id: &str) {
        // Save the current conversation
        self.save_current_conversation().await;

        let convo = if convo_id.is_empty() {
            Conversation::new_hello()
        } else {
            match self.storage.get_conversation(convo_id).await {
                Ok(convo) => convo.unwrap(),
                Err(err) => {
                    self.notice.add_message(
                        NoticeMessage::new(format!("Failed to get conversation: {}", err))
                            .with_duration(time::Duration::from_secs(5))
                            .with_type(NoticeType::Error),
                    );
                    return;
                }
            }
        };

        // Change the conversation
        self.history_screen.set_current_conversation(convo.id());
        let title = convo.title().to_string();
        self.app_state.set_conversation(convo);
        self.notice.info(format!("Switching to \"{}\"", title));
        self.input = TextArea::default().build();
        self.app_state.sync_state();
    }

    fn notice_on_error(&mut self, err: impl Into<String>) {
        self.notice.add_message(
            NoticeMessage::new(err.into())
                .with_duration(time::Duration::from_secs(5))
                .with_type(NoticeType::Error),
        );
    }

    fn on_waiting_backend(&mut self, notice: bool) -> bool {
        if self.app_state.waiting_for_backend && notice {
            self.notice.add_message(
                NoticeMessage::new("Please wait for the backend to finish!")
                    .with_duration(time::Duration::from_secs(5))
                    .with_type(NoticeType::Warning),
            );
        }
        return self.app_state.waiting_for_backend;
    }
}

fn is_line_width_sufficient(line_width: u16) -> bool {
    return line_width >= MIN_WIDTH;
}
