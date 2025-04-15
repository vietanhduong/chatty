use crate::{config::constants::FRAME_DURATION, models::Event};
use crossterm::event::{Event as CrosstermEvent, EventStream, MouseEventKind};
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time;
use tui_textarea::{Input, Key};

pub struct EventService {
    crossterm_events: EventStream,
    event_rx: mpsc::UnboundedReceiver<Event>,
    event_tx: mpsc::UnboundedSender<Event>,
}

impl EventService {
    fn handle_crossterm(&self, event: CrosstermEvent) -> Option<Event> {
        match event {
            CrosstermEvent::Paste(text) => Some(Event::KeyboardPaste(text)),
            CrosstermEvent::Mouse(mouse_event) => match mouse_event.kind {
                MouseEventKind::ScrollUp => Some(Event::UiScrollUp),
                MouseEventKind::ScrollDown => Some(Event::UiScrollDown),
                MouseEventKind::Up(button) => Some(Event::UiMouseUp {
                    button,
                    x: mouse_event.column,
                    y: mouse_event.row,
                }),
                MouseEventKind::Down(button) => Some(Event::UiMouseDown {
                    button,
                    x: mouse_event.column,
                    y: mouse_event.row,
                }),
                MouseEventKind::Drag(button) => Some(Event::UiMouseDrag {
                    button,
                    x: mouse_event.column,
                    y: mouse_event.row,
                }),
                _ => None,
            },
            CrosstermEvent::Key(key_event) => {
                let input: Input = key_event.into();
                if input.key == Key::Enter && (input.shift || input.alt) {
                    return Some(Event::KeyboardNewLine);
                }

                // Map ctrl events
                if input.ctrl {
                    match input.key {
                        Key::Char('u') => return Some(Event::UiScrollPageUp),
                        Key::Char('d') => return Some(Event::UiScrollPageDown),
                        Key::Char('q') => return Some(Event::Quit),
                        Key::Char('c') => return Some(Event::KeyboardCtrlC),
                        Key::Char('r') => return Some(Event::KeyboardCtrlR),
                        Key::Char('l') => return Some(Event::KeyboardCtrlL),
                        Key::Char('h') => return Some(Event::KeyboardCtrlH),
                        Key::Char('n') => return Some(Event::KeyboardCtrlN),
                        Key::Char('e') => return Some(Event::KeyboardCtrlE),
                        _ => return None,
                    }
                }

                match input.key {
                    Key::Esc => Some(Event::KeyboardEsc),
                    Key::F(1) => Some(Event::KeyboardF1),
                    Key::Enter => Some(Event::KeyboardEnter),
                    Key::Up => Some(Event::UiScrollUp),
                    Key::Down => Some(Event::UiScrollDown),
                    Key::MouseScrollUp => Some(Event::UiScrollPageUp),
                    Key::MouseScrollDown => Some(Event::UiScrollPageDown),
                    Key::PageUp => Some(Event::UiScrollPageUp),
                    Key::PageDown => Some(Event::UiScrollPageDown),
                    _ => Some(Event::KeyboardCharInput(input)),
                }
            }
            _ => None,
        }
    }

    pub fn event_tx(&self) -> mpsc::UnboundedSender<Event> {
        self.event_tx.clone()
    }

    pub async fn next(&mut self) -> Event {
        loop {
            let e = tokio::select! {
                event = self.event_rx.recv() => event,
                event = self.crossterm_events.next().fuse() => match event {
                    Some(Ok(input)) => self.handle_crossterm(input),
                    Some(Err(_)) => None,
                    None => None
                },
                _ = time::sleep(FRAME_DURATION) => Some(Event::UiTick)
            };

            if let Some(event) = e {
                return event;
            }
        }
    }
}

impl Default for EventService {
    fn default() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();
        Self {
            crossterm_events: EventStream::new(),
            event_rx,
            event_tx,
        }
    }
}
