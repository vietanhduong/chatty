use std::{cell::RefCell, collections::HashMap, io, rc::Rc, time};

use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use eyre::Result;
use openai_models::{
    Action, AppendMessage, BackendPrompt, Conversation, Event, Message, NoticeMessage, NoticeType,
    message::Issuer,
};
use ratatui::{
    Terminal,
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    prelude::{Backend, CrosstermBackend},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation},
};
use ratatui::{
    crossterm::{
        execute,
        terminal::{disable_raw_mode, enable_raw_mode},
    },
    text::Span,
};
use syntect::highlighting::Theme;
use tokio::sync::mpsc;

use crate::{
    app_state::{AppState, MessageAction},
    services::EventsService,
    ui::{EditScreen, HelpScreen, HistoryScreen, Loading, ModelsScreen, Notice, TextArea, helpers},
};

const MIN_WIDTH: u16 = 80;

pub struct AppInitProps {
    pub models: Vec<String>,
    pub default_model: String,
    pub conversations: HashMap<String, Conversation>,
}

pub struct App<'a> {
    action_tx: mpsc::UnboundedSender<Action>,
    events: EventsService<'a>,
    app_state: AppState<'a>,
    input: tui_textarea::TextArea<'a>,
    help_screen: HelpScreen<'a>,
    models_screen: ModelsScreen,
    edit_screen: EditScreen<'a>,
    history_screen: HistoryScreen<'a>,
    notice: Notice,
    conversations: HashMap<String, Rc<RefCell<Conversation>>>,
    loading: Loading,
    theme: &'a Theme,
}

impl<'a> App<'a> {
    pub fn new(
        theme: &'a Theme,
        action_tx: mpsc::UnboundedSender<Action>,
        event_rx: &'a mut mpsc::UnboundedReceiver<Event>,
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
            theme,
            edit_screen: EditScreen::new(action_tx.clone(), theme),
            action_tx: action_tx.clone(),
            events: EventsService::new(event_rx),
            app_state: AppState::new(default_conversation, theme),
            input: TextArea::default().build(),
            loading: Loading::new("Thinking... Press <Ctrl+c> to abort!"),
            help_screen: HelpScreen::new(),
            history_screen: HistoryScreen::default()
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

        if let Event::Notice(msg) = event {
            self.notice.add_message(msg);
            return Ok(false);
        }

        if self.help_screen.showing() {
            if self.help_screen.handle_key_event(event) {
                return Ok(true);
            }
            return Ok(false);
        }

        if self.models_screen.showing() {
            if self.models_screen.handle_key_event(event).await? {
                return Ok(true);
            }
            return Ok(false);
        }

        if self.edit_screen.showing() {
            if self.edit_screen.handle_key_event(event).await? {
                return Ok(true);
            }
            return Ok(false);
        }

        if self.history_screen.showing() {
            if self.history_screen.handle_key_event(event).await? {
                return Ok(true);
            }
            match self.history_screen.current_conversation() {
                Some(id) => {
                    let conversation_id = self.app_state.conversation.borrow().id().to_string();
                    if id != conversation_id {
                        self.app_state
                            .set_conversation(Rc::clone(self.conversations.get(&id).unwrap()));
                    }
                }
                None => {}
            }
            return Ok(false);
        }

        match event {
            Event::AbortRequest => {
                self.app_state
                    .add_message(Message::new_system("system", "Aborted!"));
            }
            Event::BackendMessage(msg) => {
                self.app_state.add_message(msg);
                self.app_state.waiting_for_backend = false;
            }
            Event::BackendPromptResponse(msg) => {
                let notify = msg.done && msg.init_conversation;
                let msg_action = self.app_state.handle_backend_response(msg);
                let mut insert = false;
                let message = match msg_action {
                    MessageAction::InsertMessage(msg) => {
                        insert = true;
                        msg
                    }
                    MessageAction::UpdateMessage(msg) => msg,
                }
                .clone();

                let conversation_id = self.app_state.conversation.borrow().id().to_string();

                self.action_tx.send(Action::AppendMessage(AppendMessage {
                    conversation_id,
                    message,
                    insert,
                }))?;

                if notify {
                    self.notice.add_message(
                        NoticeMessage::new(format!(
                            "Title: {}",
                            self.app_state.conversation.borrow().title()
                        ))
                        .with_duration(time::Duration::from_secs(5)),
                    );

                    // Save the conversation
                    self.action_tx.send(Action::UpsertConversation(
                        self.app_state.conversation.borrow().clone(),
                    ))?;
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
            Event::KeyboardCtrlQ => {
                if self.app_state.waiting_for_backend {
                    self.app_state.waiting_for_backend = false;
                    self.action_tx.send(Action::BackendAbort)?;
                }
                return Ok(true);
            }

            Event::KeyboardF1 => self.help_screen.toggle_showing(),

            Event::KeyboardCtrlN => {
                if self.app_state.waiting_for_backend {
                    self.app_state.waiting_for_backend = false;
                    self.action_tx.send(Action::BackendAbort)?;
                }

                // If the current conversation is blank (with no message)
                // we will prevent the user from creating a new conversation
                if self.app_state.conversation.borrow().len() == 1
                /* 1 for hello message */
                {
                    return Ok(false);
                }

                // Otherwise, we'll take a look the current conversation to
                // find if any blank conversation exists. If so, we will
                // use it, otherwise we will create a new one.
                // This action will prevent user create too many blank conversations
                // lead to memory leak.

                let blank_conversation = self
                    .conversations
                    .iter()
                    .filter(|(_, v)| {
                        let conversation = v.borrow();
                        conversation.len() == 1
                    })
                    .next();

                if let Some((_, conversation)) = blank_conversation {
                    self.app_state.set_conversation(Rc::clone(conversation));
                    self.history_screen
                        .set_current_conversation(conversation.borrow().id());
                    return Ok(false);
                }

                let default_conversation = Rc::new(RefCell::new(Conversation::new_hello()));
                let default_id = default_conversation.borrow().id().to_string();
                self.conversations
                    .insert(default_id.clone(), Rc::clone(&default_conversation));
                self.app_state = AppState::new(Rc::clone(&default_conversation), self.theme);
                self.history_screen.add_conversation(default_conversation);
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

                // We pop all the message from the backend
                // until we find the last message from user
                // and resubmit it to the backend

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
                        conversation.messages_mut().remove(i as usize);
                        self.app_state
                            .bubble_list
                            .remove_message_by_index(i as usize);
                        i -= 1;
                    }
                }
                self.app_state.sync_state();

                let conversation = self.app_state.conversation.borrow();
                // Resubmit the last message from user
                let last_user_msg =
                    conversation.last_message_of(Some(Issuer::User("".to_string())));

                let input_str = if let Some(msg) = last_user_msg {
                    msg.text().to_string()
                } else {
                    return Ok(false);
                };

                let model = self.models_screen.current_model();
                self.app_state.waiting_for_backend = true;
                let prompt = BackendPrompt::new(&model, input_str)
                    .with_context(conversation.context().unwrap_or_default())
                    .with_regenerate();

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
                self.app_state.add_message(msg);

                let conversation = self.app_state.conversation.borrow();
                let model = self.models_screen.current_model();

                self.app_state.waiting_for_backend = true;

                let mut prompt = BackendPrompt::new(&model, input_str)
                    .with_context(conversation.context().unwrap_or_default());

                if first {
                    prompt = prompt.with_first();
                    // Save the conversation
                    self.action_tx
                        .send(Action::UpsertConversation(conversation.clone()))?;

                    for msg in conversation.messages() {
                        self.action_tx.send(Action::AppendMessage(AppendMessage {
                            conversation_id: conversation.id().to_string(),
                            message: msg.clone(),
                            insert: true,
                        }))?;
                    }

                    self.history_screen
                        .set_current_conversation(conversation.id());
                }

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
                    Paragraph::new(helpers::split_to_lines(
                        format!(
                            "I'm too small, make me bigger! I need at least {} cells (current: {})",
                            MIN_WIDTH, current_width
                        )
                        .as_str()
                        .split(' ')
                        .map(|s| Span::raw(s))
                        .collect::<Vec<_>>(),
                        current_width as usize,
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
                .render(f, helpers::popup_area(f.area(), 40, 30));

            self.models_screen
                .render(f, helpers::popup_area(f.area(), 30, 60));

            self.edit_screen
                .render(f, helpers::popup_area(f.area(), 70, 90));
            self.history_screen
                .render(f, helpers::popup_area(f.area(), 40, 90));

            self.notice.render(f, helpers::notice_area(f.area(), 30));
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
}

fn is_line_width_sufficient(line_width: u16) -> bool {
    return line_width >= MIN_WIDTH;
}
