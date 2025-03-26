use std::{cell::RefCell, collections::HashMap, io, rc::Rc, sync::Arc, time};

use crate::backend::Compressor;
use crate::models::Model;
use crate::models::{
    Action, BackendPrompt, Conversation, Event, Message, NoticeMessage, NoticeType, message::Issuer,
};
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

    // FIXME: Holding the entire conversation in memory is not ideal
    // but for now, we will do it this way
    conversations: HashMap<String, Rc<RefCell<Conversation>>>,

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
        let mut conversations: HashMap<String, Rc<RefCell<Conversation>>> = init_props
            .conversations
            .into_iter()
            .map(|(k, v)| (k, Rc::new(RefCell::new(v))))
            .collect();

        let default_conversation = Rc::new(RefCell::new(Conversation::new_hello()));
        let default_id = default_conversation.borrow().id().to_string();
        conversations.insert(default_id.clone(), Rc::clone(&default_conversation));

        App {
            event_tx: event_tx.clone(),
            compressor,
            storage: storage.clone(),
            edit_screen: EditScreen::new(action_tx.clone(), theme),
            action_tx: action_tx.clone(),
            events: EventsService::new(event_rx),
            app_state: AppState::new(default_conversation, theme),
            input: TextArea::default().build(),
            loading: Loading::new(vec![
                span!("Thinking... Press ").gray(),
                span!("Ctrl+c").green().bold(),
                span!(" to abort!").gray(),
            ]),
            help_screen: HelpScreen::new(),
            history_screen: HistoryScreen::new(event_tx.clone(), storage)
                .with_conversations(conversations.iter().map(|(_, v)| Rc::clone(v)).collect())
                .with_current_conversation(default_id),
            conversations,
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

        if let Event::Quit = event {
            self.save_last_message().await?;
        }

        if let Event::ConversationDeleted(id) = event {
            self.conversations.remove(&id);
            self.history_screen.remove_conversation(&id);
            if self.history_screen.current_conversation().is_none() {
                let conversation = self.get_default_or_create_conversation();
                self.change_conversation(conversation);
            }
            return Ok(false);
        }

        if let Event::ConversationUpdated(id) = event {
            let convo = self.storage.get_conversation(&id).await?;
            if let Some(convo) = convo {
                let rc = Rc::new(RefCell::new(convo));
                self.conversations.insert(id.clone(), rc.clone());
                if self.app_state.conversation.borrow().id() == id {
                    self.change_conversation(rc);
                }
            }
            return Ok(false);
        }

        if let Event::SetConversation(id) = event {
            if self.app_state.conversation.borrow().id() == id {
                return Ok(false);
            }
            self.change_conversation(Rc::clone(self.conversations.get(&id).unwrap()));
            self.notice.info(format!(
                "Switched to: \"{}\"",
                self.app_state.conversation.borrow().title()
            ));
            return Ok(false);
        }

        if let Event::Notice(msg) = event {
            self.notice.add_message(msg);
            return Ok(false);
        }

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

        match event {
            Event::ModelChanged(model) => {
                self.models_screen.set_current_model(&model);
                self.notice.info(format!("Changed model to \"{}\"", model));
                return Ok(false);
            }

            Event::AbortRequest => {
                let convo_id = self.app_state.conversation.borrow().id().to_string();
                let message = Message::new_system("system", "Aborted!");
                self.app_state.add_message(message.clone());
                self.storage.upsert_message(&convo_id, message).await?;
            }

            Event::BackendMessage(msg) => {
                let convo_id = self.app_state.conversation.borrow().id().to_string();
                self.app_state.add_message(msg.clone());
                self.storage.upsert_message(&convo_id, msg).await?;
                self.app_state.waiting_for_backend = false;
            }

            Event::BackendPromptResponse(msg) => {
                let notify = msg.done && msg.init_conversation;
                let done = msg.done;
                self.app_state.handle_backend_response(&msg);

                if notify {
                    self.notice.add_message(
                        NoticeMessage::new(format!(
                            "Updated Title: \"{}\"",
                            self.app_state.conversation.borrow().title()
                        ))
                        .with_duration(time::Duration::from_secs(5)),
                    );

                    // Upsert the conversation to the storage
                    self.storage
                        .upsert_conversation(self.app_state.conversation.borrow().clone())
                        .await?;
                }

                if !done {
                    return Ok(false);
                }

                let mut conversation = self.app_state.conversation.borrow_mut();
                let conversation_id = conversation.id().to_string();

                if let Some(ref usage) = msg.usage {
                    if let Some(msg) = conversation.last_message_of_mut(Some(Issuer::user())) {
                        msg.set_token_count(usage.prompt_tokens);
                        self.storage
                            .upsert_message(&conversation_id, msg.clone())
                            .await?;
                    }

                    if let Some(msg) = conversation.last_message_of_mut(Some(Issuer::system())) {
                        msg.set_token_count(usage.completion_tokens);
                    }

                    // If the usage is not empty, show it
                    self.notice.add_message(
                        NoticeMessage::info(format!("Usage: {}", usage.to_string()))
                            .with_duration(time::Duration::from_secs(6)),
                    );
                }

                // Upsert message to the storage
                self.storage
                    .upsert_message(
                        &conversation_id,
                        conversation.last_message().unwrap().clone(),
                    )
                    .await?;

                // If the conversation should be compressed, we will process it
                // in the background and notify the app when it's done to fetch
                // the context and update the conversation. This will mitigate
                // impact to the current conversation.
                if self.compressor.should_compress(&conversation) {
                    self.handle_convo_compress(&conversation_id);
                }
            }
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
            Event::Quit => {
                if self.app_state.waiting_for_backend {
                    self.app_state.waiting_for_backend = false;
                    self.action_tx.send(Action::BackendAbort)?;
                }
                return Ok(true);
            }

            Event::KeyboardF1 => self.help_screen.toggle_showing(),

            Event::KeyboardCtrlN => {
                self.handle_new_conversation();
                return Ok(false);
            }

            Event::KeyboardCtrlH => {
                if self.app_state.waiting_for_backend {
                    self.notice.add_message(
                        NoticeMessage::new("Please wait for the backend to finish!".to_string())
                            .with_duration(time::Duration::from_secs(5))
                            .with_type(NoticeType::Warning),
                    );
                    return Ok(false);
                }
                self.history_screen.toggle_showing();
            }

            Event::KeyboardCtrlL => self.models_screen.toggle_showing(),

            Event::KeyboardCtrlE => {
                if self.app_state.waiting_for_backend {
                    return Ok(false);
                }
                self.edit_screen
                    .set_messages(self.app_state.conversation.borrow().messages());
                self.edit_screen.toggle_showing();
            }

            Event::KeyboardCtrlR => {
                if self.app_state.waiting_for_backend {
                    return Ok(false);
                }

                // Rebuild the conversation by removing all the messages from the backend
                // and resubmit the last message from user
                {
                    let mut conversation = self.app_state.conversation.borrow_mut();

                    let mut i = conversation.len() as i32 - 1;
                    if i == 0 {
                        // Welcome message, nothing to do
                        return Ok(false);
                    }

                    while i >= 0 {
                        if !conversation.messages()[i as usize].is_system() {
                            break;
                        }
                        let con = conversation.messages_mut().remove(i as usize);
                        self.app_state
                            .bubble_list
                            .remove_message_by_index(i as usize);
                        // Tell the storage to remove the message
                        self.storage.delete_messsage(con.id()).await?;
                        i -= 1;
                    }
                }
                self.app_state.sync_state();
                self.app_state.scroll.last();

                let conversation = self.app_state.conversation.borrow();
                // Resubmit the last message from user
                let last_user_msg = conversation.last_message_of(Some(Issuer::user()));

                let input_str = if let Some(msg) = last_user_msg {
                    msg.text().to_string()
                } else {
                    return Ok(false);
                };

                let model = self.models_screen.current_model();
                self.app_state.waiting_for_backend = true;
                let prompt = BackendPrompt::new(input_str)
                    .with_model(model)
                    .with_context(conversation.build_context());

                self.action_tx.send(Action::BackendRequest(prompt))?;
            }

            Event::KeyboardPaste(text) => {
                self.input.set_yank_text(text.replace('\r', "\n"));
                self.input.paste();
            }

            Event::KeyboardAltEnter => {
                if self.app_state.waiting_for_backend {}
                self.input.insert_newline();
            }

            Event::KeyboardEnter => {
                if self.app_state.waiting_for_backend {
                    return Ok(false);
                }
                let input_str = &self.input.lines().join("\n");
                if input_str.is_empty() {
                    return Ok(false);
                }

                let first = self.app_state.conversation.borrow().len() < 2;

                let msg = Message::new_user("user", input_str);
                self.input = TextArea::default().build();
                self.app_state.add_message(msg.clone());

                let conversation_id = self.app_state.conversation.borrow().id().to_string();
                let model = self.models_screen.current_model();

                self.app_state.waiting_for_backend = true;

                let prompt = BackendPrompt::new(input_str)
                    .with_context(self.app_state.conversation.borrow().build_context())
                    .with_model(model);

                if first {
                    self.history_screen
                        .set_current_conversation(conversation_id.clone());

                    self.storage
                        .upsert_conversation(self.app_state.conversation.borrow().clone())
                        .await?;

                    // Save the first message to the storage
                    self.storage
                        .upsert_message(
                            &conversation_id,
                            self.app_state.conversation.borrow().messages()[0].clone(),
                        )
                        .await?;
                }

                self.storage
                    .upsert_message(&conversation_id, msg.clone())
                    .await?;
                self.action_tx.send(Action::BackendRequest(prompt))?;
            }

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

    fn handle_new_conversation(&mut self) {
        if self.app_state.waiting_for_backend {
            self.app_state.waiting_for_backend = false;
            if let Err(err) = self.action_tx.send(Action::BackendAbort) {
                self.notice.add_message(
                    NoticeMessage::new(format!("Failed to abort: {}", err))
                        .with_duration(time::Duration::from_secs(5))
                        .with_type(NoticeType::Error),
                );
            }
        }

        if self.app_state.conversation.borrow().len() < 2 {
            return;
        }

        let conversation = self.get_default_or_create_conversation();
        self.change_conversation(conversation);
    }

    fn get_default_or_create_conversation(&mut self) -> Rc<RefCell<Conversation>> {
        let blank_conversation = self
            .conversations
            .iter()
            .filter(|(_, v)| v.borrow().len() < 2)
            .next();

        if let Some((_, conversation)) = blank_conversation {
            return Rc::clone(conversation);
        }

        let default_conversation = Rc::new(RefCell::new(Conversation::new_hello()));
        let default_id = default_conversation.borrow().id().to_string();
        self.conversations
            .insert(default_id.clone(), Rc::clone(&default_conversation));
        self.history_screen
            .add_conversation(Rc::clone(&default_conversation));
        default_conversation
    }

    async fn save_last_message(&mut self) -> Result<()> {
        if !self.app_state.waiting_for_backend {
            // If no message is waiting, we can simply return the process
            return Ok(());
        }
        // Otherwise, let save the last message in the conversation
        let last_message = self.app_state.last_message().unwrap();
        self.storage
            .upsert_message(self.app_state.conversation.borrow().id(), last_message)
            .await?;
        Ok(())
    }

    fn change_conversation(&mut self, new_con: Rc<RefCell<Conversation>>) {
        // Change the conversation
        self.history_screen
            .set_current_conversation(new_con.borrow().id());
        self.app_state.set_conversation(new_con);
        self.input = TextArea::default().build();
        self.app_state.sync_state();
    }
}

fn is_line_width_sufficient(line_width: u16) -> bool {
    return line_width >= MIN_WIDTH;
}
