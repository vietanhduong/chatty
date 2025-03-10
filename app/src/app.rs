use std::io;

use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use eyre::Result;
use openai_models::{Action, BackendPrompt, Event, Message};
use ratatui::{
    Frame, Terminal,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    prelude::{Backend, CrosstermBackend},
    style::Modifier,
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation},
};
use ratatui::{
    crossterm::{
        execute,
        terminal::{disable_raw_mode, enable_raw_mode},
    },
    style::Style,
};
use tokio::sync::mpsc;

use crate::{
    app_state::AppState, events::EventsService, instructions::render_instruction,
    textarea::TextArea,
};

pub async fn start(
    action_tx: mpsc::UnboundedSender<Action>,
    event_rx: mpsc::UnboundedReceiver<Event>,
) -> Result<()> {
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
    start_loop(&mut terminal, action_tx, event_rx).await?;

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

async fn start_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    action_tx: mpsc::UnboundedSender<Action>,
    event_rx: mpsc::UnboundedReceiver<Event>,
) -> Result<()> {
    let mut events = EventsService::new(event_rx);
    let mut textarea = TextArea::default().build();

    let mut app_state = AppState::new().await;

    let loading = Loading::new("Thinking...");

    loop {
        terminal.draw(|frame| {
            if !is_line_width_sufficient(frame.area().width) {
                frame.render_widget(
                    Paragraph::new("I'm too small, make me bigger!").alignment(Alignment::Left),
                    frame.area(),
                );
                return;
            }

            let textarea_len = (textarea.lines().len() + 2).try_into().unwrap();
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![
                    Constraint::Min(1),
                    Constraint::Max(textarea_len),
                    Constraint::Length(1),
                ])
                .split(frame.area());

            if layout[0].width as usize != app_state.last_known_width
                || layout[0].height as usize != app_state.last_known_height
            {
                app_state.set_rect(layout[0]);
            }

            app_state.bubble_list.render(
                layout[0],
                frame.buffer_mut(),
                app_state.scroll.position.try_into().unwrap(),
            );

            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                layout[0].inner(Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut app_state.scroll.scrollbar_state,
            );

            render_instruction(frame, layout[2]);
            if app_state.waiting_for_backend {
                loading.render(frame, layout[1]);
            } else {
                frame.render_widget(&textarea, layout[1]);
            }
        })?;

        match events.next().await? {
            Event::BackendMessage(msg) => {
                app_state.add_message(msg);
                app_state.waiting_for_backend = false;
            }
            Event::BackendPromptResponse(msg) => {
                app_state.handle_backend_response(msg);
            }
            Event::KeyboardCharInput(c) => {
                if app_state.waiting_for_backend {
                    continue;
                }
                textarea.input(c);
            }
            Event::KeyboardCtrlC => {
                // TODO: handle abort backend
            }
            Event::KeyboardCtrlQ => {
                if app_state.waiting_for_backend {
                    app_state.waiting_for_backend = false;
                    action_tx.send(Action::BackendAbort)?;
                }
                return Ok(());
            }

            Event::KeyboardCtrlH => {
                // TODO: Handle toggle open History
            }

            Event::KeyboardCtrlR => {
                // TODO: Handle regenerate message
            }

            Event::KeyboardPaste(text) => {
                textarea.set_yank_text(text.replace('\r', "\n"));
                textarea.paste();
            }

            Event::KeyboardAltEnter => {
                if app_state.waiting_for_backend {
                    continue;
                }
                textarea.insert_newline();
            }

            Event::KeyboardEnter => {
                if app_state.waiting_for_backend {
                    continue;
                }
                let input_str = &textarea.lines().join("\n");
                if input_str.is_empty() {
                    continue;
                }

                let msg = Message::new_user("user", input_str);
                textarea = TextArea::default().build();
                app_state.add_message(msg);

                app_state.waiting_for_backend = true;

                let mut prompt = BackendPrompt::new(input_str);

                if !app_state.backend_context.is_empty() {
                    prompt = prompt.with_context(&app_state.backend_context);
                }
                action_tx.send(Action::BackendRequest(prompt))?;
            }

            Event::UiTick => {
                continue;
            }
            Event::UiScrollDown => app_state.scroll.down(),
            Event::UiScrollUp => app_state.scroll.up(),
            Event::UiScrollPageDown => app_state.scroll.page_down(),
            Event::UiScrollPageUp => app_state.scroll.page_up(),
        }
    }
}

fn is_line_width_sufficient(line_width: u16) -> bool {
    let min_width = (8 + 5) as i32;
    let trimmed_line_width = ((line_width as f32 * (1.0 - 0.04)).ceil()) as i32;
    return trimmed_line_width >= min_width;
}

#[derive(Default)]
pub struct Loading(String);

impl Loading {
    pub fn new(text: &str) -> Self {
        Self(text.to_string())
    }

    fn text(&self) -> &str {
        if self.0.is_empty() {
            "Loading..."
        } else {
            &self.0
        }
    }

    pub fn render(&self, frame: &mut Frame, rect: Rect) {
        frame.render_widget(
            Paragraph::new(self.text())
                .style(Style {
                    add_modifier: Modifier::ITALIC,
                    ..Default::default()
                })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .padding(Padding::new(1, 1, 0, 0)),
                ),
            rect,
        );
    }
}
