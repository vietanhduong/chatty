use std::sync::Arc;

use eyre::Result;
use openai_backend::ArcBackend;
use openai_models::{Action, BackendPrompt, Event, Message, NoticeMessage, NoticeType};
use tokio::{sync::mpsc, task::JoinHandle};

use super::ClipboardService;

// use crate::clipboard::ClipboardService;

pub struct ActionService<'a> {
    event_tx: mpsc::UnboundedSender<Event>,
    action_rx: &'a mut mpsc::UnboundedReceiver<Action>,
    backend: ArcBackend,
}

async fn completions(
    backend: &ArcBackend,
    prompt: BackendPrompt,
    event_tx: &mpsc::UnboundedSender<Event>,
) -> Result<()> {
    let lock = backend.lock().await;
    lock.get_completion(prompt, event_tx).await?;
    Ok(())
}

fn worker_error(err: eyre::Error, event_tx: &mpsc::UnboundedSender<Event>) -> Result<()> {
    event_tx.send(Event::BackendMessage(Message::new_system(
        "system",
        format!("Error: Backend failed with the following error: \n\n {err:?}"),
    )))?;

    Ok(())
}

impl ActionService<'_> {
    pub fn new(
        event_tx: mpsc::UnboundedSender<Event>,
        action_rx: &'_ mut mpsc::UnboundedReceiver<Action>,
        backend: ArcBackend,
    ) -> ActionService<'_> {
        ActionService {
            event_tx,
            action_rx,
            backend,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let mut worker: JoinHandle<Result<()>> = tokio::spawn(async {
            return Ok(());
        });

        loop {
            let event = self.action_rx.recv().await;
            if event.is_none() {
                continue;
            }

            let worker_tx = self.event_tx.clone();
            match event.unwrap() {
                Action::BackendAbort => {
                    worker.abort();
                    worker_tx.send(Event::AbortRequest)?;
                }

                Action::BackendRequest(prompt) => {
                    let backend = Arc::clone(&self.backend);
                    worker = tokio::spawn(async move {
                        if let Err(err) = completions(&backend, prompt, &worker_tx).await {
                            worker_error(err, &worker_tx)?;
                        }
                        Ok(())
                    })
                }
                Action::CopyMessages(messages) => {
                    if let Err(err) = self.copy_messages(messages) {
                        log::error!("Failed to copy messages: {}", err);
                        self.event_tx.send(Event::Notice(
                            NoticeMessage::new(format!("Failed to copy messages: {}", err))
                                .with_type(NoticeType::Error),
                        ))?;
                    }
                }
            }
        }
    }

    fn copy_messages(&self, messages: Vec<Message>) -> Result<()> {
        let mut payload = messages[0].text().to_string();
        if messages.len() > 1 {
            payload = messages
                .iter()
                .map(|msg| msg.text().to_string())
                .collect::<Vec<String>>()
                .join("\n\n")
        }

        if let Err(err) = ClipboardService::set(payload) {
            log::error!("Failed to copy to clipboard: {}", err);
            self.event_tx.send(Event::Notice(
                NoticeMessage::new(format!("Failed to copy to clipboard: {}", err))
                    .with_type(NoticeType::Error),
            ))?;

            return Ok(());
        }

        self.event_tx
            .send(Event::Notice(NoticeMessage::new("Copied to clipboard!")))?;

        Ok(())
    }
}
