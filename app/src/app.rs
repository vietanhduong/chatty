use std::io;

use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use eyre::{Context, Result};
use openai_backend::ArcBackend;
use openai_models::{Action, BackendPrompt, Event, Message, message::Issuer};
use ratatui::crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    layout::{Alignment, Constraint, Direction, Layout, Margin},
    prelude::{Backend, CrosstermBackend},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation},
};
use syntect::highlighting::Theme;
use tokio::sync::mpsc;

use crate::{
    app_state::AppState,
    services::EventsService,
    ui::{HelpScreen, Loading, ModelsScreen, TextArea, bubble},
};

pub struct App<'a> {
    action_tx: mpsc::UnboundedSender<Action>,
    events: EventsService<'a>,
    app_state: AppState<'a>,
    input: tui_textarea::TextArea<'a>,
    help_screen: HelpScreen<'a>,
    models_screen: ModelsScreen,

    loading: Loading,
    theme: &'a Theme,
}

impl<'a> App<'_> {
    pub fn new(
        backend: ArcBackend,
        theme: &'a Theme,
        action_tx: mpsc::UnboundedSender<Action>,
        event_rx: &'a mut mpsc::UnboundedReceiver<Event>,
    ) -> App<'a> {
        App {
            theme,
            action_tx,
            events: EventsService::new(event_rx),
            app_state: AppState::new(theme),
            input: TextArea::default().build(),
            loading: Loading::new("Thinking... Press <Ctrl+c> to abort!"),
            help_screen: HelpScreen::new(),
            models_screen: ModelsScreen::new(backend),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let stdout = io::stdout();
        let mut stdout = stdout.lock();

        // Prefetch the models in the background
        self.models_screen
            .fetch_models()
            .await
            .wrap_err("fetching models")?;

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

        if self.help_screen.showing() {
            if self.help_screen.handle_key_event(event) {
                // If true, stop the process
                return Ok(true);
            }
            return Ok(false);
        }

        if self.models_screen.showing() {
            if self.models_screen.handle_key_event(event).await? {
                // If true, stop the process
                return Ok(true);
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
                self.app_state.handle_backend_response(msg);
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
                self.app_state = AppState::new(self.theme);
                return Ok(false);
            }
            Event::KeyboardCtrlH => {
                // TODO: Handle toggle open History
            }

            Event::KeyboardCtrlL => {
                if self.app_state.waiting_for_backend {
                    return Ok(false);
                }
                self.models_screen.toggle_showing()
            }

            Event::KeyboardCtrlR => {
                if self.app_state.waiting_for_backend {
                    return Ok(false);
                }

                // We pop all the message from the backend
                // until we find the last message from user
                // and resubmit it to the backend

                let mut i = self.app_state.messages.len() as i32 - 1;
                if i == 0 {
                    // Welcome message, nothing to do
                    return Ok(false);
                }

                while i >= 0 {
                    if !self.app_state.messages[i as usize].is_system() {
                        break;
                    }
                    self.app_state.messages.remove(i as usize);
                    self.app_state
                        .bubble_list
                        .remove_message_by_index(i as usize);
                    i -= 1;
                }
                self.app_state.sync_state();

                // Resubmit the last message from user
                let last_user_msg = self
                    .app_state
                    .last_message_of(Some(Issuer::User("".to_string())));

                let input_str = if let Some(msg) = last_user_msg {
                    msg.text().to_string()
                } else {
                    return Ok(false);
                };

                let msg_id = last_user_msg.unwrap().id().to_string();

                self.app_state.waiting_for_backend = true;
                let mut prompt = BackendPrompt::new(input_str);

                let context: Vec<Message> = self
                    .app_state
                    .build_context_for(&msg_id)
                    .into_iter()
                    .cloned()
                    .collect();

                if !context.is_empty() {
                    prompt = prompt.with_context(context);
                }
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

                let msg = Message::new_user("user", input_str);
                let msg_id = msg.id().to_string();
                self.input = TextArea::default().build();
                self.app_state.add_message(msg);

                self.app_state.waiting_for_backend = true;

                let mut prompt = BackendPrompt::new(input_str);

                let context: Vec<Message> = self
                    .app_state
                    .build_context_for(&msg_id)
                    .into_iter()
                    .cloned()
                    .collect();

                if !context.is_empty() {
                    prompt = prompt.with_context(context);
                }
                self.action_tx.send(Action::BackendRequest(prompt))?;
            }

            Event::UiTick => {
                return Ok(false);
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
        terminal.draw(|frame| {
            if !is_line_width_sufficient(frame.area().width) {
                frame.render_widget(
                    Paragraph::new("I'm too small, make me bigger!").alignment(Alignment::Left),
                    frame.area(),
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
                .split(frame.area());

            if layout[0].width as usize != self.app_state.last_known_width
                || layout[0].height as usize != self.app_state.last_known_height
            {
                self.app_state.set_rect(layout[0]);
            }

            self.app_state.bubble_list.render(
                layout[0],
                frame.buffer_mut(),
                self.app_state.scroll.position.try_into().unwrap(),
            );

            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .end_symbol(None)
                    .begin_symbol(None),
                layout[0].inner(Margin {
                    vertical: 1,
                    horizontal: 1,
                }),
                &mut self.app_state.scroll.scrollbar_state,
            );

            self.help_screen.render_help_line(frame, layout[2]);
            if self.app_state.waiting_for_backend {
                self.loading.render(frame, layout[1]);
            } else {
                frame.render_widget(&self.input, layout[1]);
            }
            if self.help_screen.showing() {
                self.help_screen.render(frame, frame.area())
            }
            if self.models_screen.showing() {
                self.models_screen.render(frame, frame.area());
            }
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
    let min_width = (bubble::DEFAULT_PADDING + bubble::DEFAULT_BORDER_ELEMENTS_LEN) as i32;
    let trimmed_line_width =
        ((line_width as f32 * (1.0 - bubble::DEFAULT_OUTER_PADDING_PERCENTAGE)).ceil()) as i32;
    return trimmed_line_width >= min_width;
}
