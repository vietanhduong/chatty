use crossterm::event::{Event as CrosstermEvent, EventStream, MouseEventKind};
use eyre::Result;
use futures::{FutureExt, StreamExt};
use openai_models::Event;
use tokio::sync::mpsc;
use tokio::time;
use tui_textarea::{Input, Key};

pub struct EventsService<'a> {
    crossterm_events: EventStream,
    events: &'a mut mpsc::UnboundedReceiver<Event>,
}

impl<'a> EventsService<'_> {
    pub fn new(events: &'a mut mpsc::UnboundedReceiver<Event>) -> EventsService<'a> {
        EventsService {
            crossterm_events: EventStream::new(),
            events,
        }
    }

    fn handle_crossterm(&self, event: CrosstermEvent) -> Option<Event> {
        match event {
            CrosstermEvent::Paste(text) => Some(Event::KeyboardPaste(text)),
            CrosstermEvent::Mouse(mouse_event) => match mouse_event.kind {
                MouseEventKind::ScrollUp => Some(Event::UiScrollUp),
                MouseEventKind::ScrollDown => Some(Event::UiScrollDown),
                _ => None,
            },
            CrosstermEvent::Key(key_event) => {
                let input: Input = key_event.into();

                if input.alt && input.key == Key::Enter {
                    return Some(Event::KeyboardAltEnter);
                }

                // Map ctrl events
                if input.ctrl {
                    match input.key {
                        Key::Char('u') => return Some(Event::UiScrollUp),
                        Key::Char('d') => return Some(Event::UiScrollDown),
                        Key::Char('q') => return Some(Event::KeyboardCtrlQ),
                        Key::Char('c') => return Some(Event::KeyboardCtrlC),
                        Key::Char('r') => return Some(Event::KeyboardCtrlR),
                        Key::Char('l') => return Some(Event::KeyboardCtrlL),
                        Key::Char('h') => return Some(Event::KeyboardCtrlH),
                        Key::Char('n') => return Some(Event::KeyboardCtrlN),
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

    pub async fn next(&mut self) -> Result<Event> {
        loop {
            let e = tokio::select! {
                event = self.events.recv() => event,
                event = self.crossterm_events.next().fuse() => match event {
                    Some(Ok(input)) => self.handle_crossterm(input),
                    Some(Err(_)) => None,
                    None => None
                },
                _ = time::sleep(time::Duration::from_millis(500)) => Some(Event::UiTick)
            };

            if let Some(event) = e {
                return Ok(event);
            }
        }
    }
}
