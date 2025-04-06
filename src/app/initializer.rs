use crossterm::event::Event as CrosstermEvent;
use crossterm::event::EventStream;
use eyre::Result;
use futures::{FutureExt, StreamExt};
use once_cell::sync::OnceCell;
use ratatui::Terminal;
use ratatui::prelude::Backend;
use ratatui::prelude::CrosstermBackend;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui_macros::span;
use tokio::{sync::mpsc, time};
use tui_textarea::Input;
use tui_textarea::Key;

use crate::models::NoticeKind;
use crate::models::NoticeMessage;
use crate::{
    config::constants::FRAME_DURATION,
    models::task::{Task, TaskEvent},
};

use super::destruct_terminal;
use super::init_terminal;
use super::ui::utils;

static SENDER: OnceCell<mpsc::UnboundedSender<TaskEvent>> = OnceCell::new();

#[derive(Debug)]
enum Event {
    Task {
        inner: Task,
        created_at: chrono::DateTime<chrono::Utc>,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
        success: bool,
    },
    Notice {
        inner: NoticeMessage,
    },
}

#[macro_export]
macro_rules! task_success {
    ($id:expr) => {
        $crate::app::Initializer::complete_task($id, None, true)
    };
    ($id:expr, $msg:expr) => {
        $crate::app::Initializer::complete_task($id, Some($msg), true)
    };
}

#[macro_export]
macro_rules! task_failure {
    ($id:expr) => {
        $crate::app::Initializer::complete_task($id, None, false)
    };
    ($id:expr, $msg:expr) => {
        $crate::app::Initializer::complete_task($id, Some($msg), false)
    };
}

pub struct Initializer {
    crossterm_events: EventStream,
    task_rx: mpsc::UnboundedReceiver<TaskEvent>,
    messages: Vec<Event>,
}

impl Initializer {
    pub fn ready() -> bool {
        SENDER.get().is_some()
    }

    pub fn add_notice(msg: NoticeMessage) {
        let task = TaskEvent::AddNotice(msg);
        if let Some(sender) = SENDER.get() {
            let _ = sender.send(task);
        }
    }

    pub fn add_task(id: &str, name: &str) {
        log::debug!("Initializing task: {}", name);
        let task = TaskEvent::AddTask(Task {
            id: id.to_string(),
            message: name.to_string(),
        });
        if let Some(sender) = SENDER.get() {
            let _ = sender.send(task);
        }
    }

    pub fn complete() {
        let task = TaskEvent::Complete;
        if let Some(sender) = SENDER.get() {
            let _ = sender.send(task);
        }
    }

    pub fn is_complete(&self) -> bool {
        self.messages.iter().any(|t| {
            matches!(
                t,
                Event::Task {
                    completed_at: Some(_),
                    ..
                }
            )
        })
    }

    pub fn complete_task(id: &str, message: Option<String>, success: bool) {
        let task = TaskEvent::CompleteTask {
            id: id.to_string(),
            suffix_message: message,
            success,
        };
        if let Some(sender) = SENDER.get() {
            let _ = sender.send(task);
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        init_terminal()?;
        let term_backend = CrosstermBackend::new(std::io::stdout());
        let mut terminal = Terminal::new(term_backend)?;
        let result = self.start_loop(&mut terminal).await;
        destruct_terminal();
        terminal.show_cursor()?;
        result
    }

    async fn start_loop<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            let event = self.next_event().await;
            if let TaskEvent::UiTick = event {
                self.render(terminal)?;
                continue;
            }
            if self.handle_event(event) {
                return Ok(());
            }
        }
    }

    async fn next_event(&mut self) -> TaskEvent {
        loop {
            let event = tokio::select! {
                event = self.task_rx.recv() =>event,
                event = self.crossterm_events.next().fuse() => match event {
                    Some(Ok(CrosstermEvent::Key(key_event))) => Some(TaskEvent::CrosstermKey(key_event)),
                    Some(Err(_)) => None,
                    _ => None
                },
                _ = time::sleep(FRAME_DURATION) => Some(TaskEvent::UiTick)
            };

            if let Some(event) = event {
                return event;
            }
        }
    }

    fn handle_event(&mut self, event: TaskEvent) -> bool {
        if let TaskEvent::UiTick = event {
            return false;
        }
        match event {
            TaskEvent::AddTask(task) => {
                let mut task = task;
                if !task.message.ends_with("...") {
                    task.message = format!("{}...", task.message);
                }
                self.messages.push(Event::Task {
                    inner: task,
                    created_at: chrono::Utc::now(),
                    completed_at: None,
                    success: false,
                });
            }
            TaskEvent::AddNotice(notice) => {
                self.messages.push(Event::Notice { inner: notice });
            }
            TaskEvent::CompleteTask {
                id,
                suffix_message,
                success,
            } => self.handle_task_complete(&id, suffix_message, success),
            TaskEvent::Complete => {
                return true;
            }
            TaskEvent::CrosstermKey(key) => {
                let input: Input = key.into();
                if input.ctrl && (input.key == Key::Char('c') || input.key == Key::Char('q')) {
                    destruct_terminal();
                    std::process::exit(0);
                }
            }
            _ => {}
        }
        false
    }

    fn handle_task_complete(&mut self, task_id: &str, message: Option<String>, task_success: bool) {
        let task = self
            .messages
            .iter_mut()
            .find(|t| matches!(t, Event::Task { inner, .. } if inner.id == task_id));

        let task = match task {
            Some(task) => task,
            None => return,
        };

        if let Event::Task {
            inner,
            completed_at,
            success,
            ..
        } = task
        {
            *completed_at = Some(chrono::Utc::now());
            if let Some(message) = message {
                inner.message = format!("{} {}", inner.message, message);
            }
            *success = task_success;
            log::debug!(
                "Completing task: {} (success: {})",
                inner.message,
                task_success
            );
        }
    }

    fn render<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        terminal.draw(|f| {
            let area = f.area();
            let popup_area = utils::popup_area(area, 60, 70);

            let tasks = self
                .messages
                .iter()
                .map(|t| t.to_line())
                .collect::<Vec<_>>();

            let paragraph = Paragraph::new(tasks).block(Block::default());
            paragraph.render(popup_area, f.buffer_mut());
        })?;
        Ok(())
    }
}

impl Default for Initializer {
    fn default() -> Self {
        let (task_tx, task_rx) = mpsc::unbounded_channel::<TaskEvent>();
        SENDER.set(task_tx).unwrap();

        Self {
            crossterm_events: EventStream::new(),
            task_rx,
            messages: vec![],
        }
    }
}

impl Event {
    fn to_line<'b>(&self) -> Line<'b> {
        match self {
            Event::Task {
                inner,
                completed_at,
                created_at,
                success,
                ..
            } => {
                let mut spans = vec![];
                let mut message = inner.message.clone();
                if let Some(completed_at) = completed_at {
                    if *success {
                        spans.push(span!("✓ ").green());
                    } else {
                        spans.push(span!("✗ ").red());
                    }
                    let took = completed_at
                        .signed_duration_since(*created_at)
                        .to_std()
                        .unwrap_or_default();
                    message
                        .push_str(format!(" completed! (took {}(s))", took.as_secs_f64()).as_str());
                } else {
                    spans.push(span!("? ").yellow());
                }
                spans.push(span!(message));
                Line::from(spans)
            }
            Event::Notice { inner, .. } => {
                let mut spans = vec![];
                match inner.kind() {
                    NoticeKind::Info => {
                        spans.push(span!("[i] ").fg(inner.kind().text_color()));
                    }
                    NoticeKind::Warning => {
                        spans.push(span!("[!] ").fg(inner.kind().text_color()));
                    }
                    NoticeKind::Error => {
                        spans.push(span!("[x] ").fg(inner.kind().text_color()));
                    }
                }
                spans.push(span!(inner.message().to_string()));
                Line::from(spans)
            }
        }
    }
}
