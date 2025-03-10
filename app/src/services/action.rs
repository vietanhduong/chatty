use std::sync::Arc;

use eyre::{Ok, Result};
use openai_backend::BoxedBackend;
use openai_models::{Action, BackendPrompt, Event, Message};
use tokio::{sync::mpsc, task::JoinHandle};

// use crate::clipboard::ClipboardService;

pub struct ActionService<'a> {
    event_tx: mpsc::UnboundedSender<Event>,
    action_rx: &'a mut mpsc::UnboundedReceiver<Action>,
    backend: Arc<BoxedBackend>,
}

async fn completions(
    backend: &BoxedBackend,
    prompt: BackendPrompt,
    event_tx: &mpsc::UnboundedSender<Event>,
) -> Result<()> {
    backend.get_completion(prompt, event_tx).await?;
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
        backend: Arc<BoxedBackend>,
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
                Action::CopyMessages(_messages) => {}
            }
        }
    }

    // fn copy_messages(&self, messages: Vec<Message>) -> Result<()> {
    //     let mut payload = messages[0].text().to_string();
    //     if messages.len() > 1 {
    //         payload = messages
    //             .iter()
    //             .map(|msg| format!("{}: {}", msg.author(), msg.text()))
    //             .collect::<Vec<String>>()
    //             .join("\n\n")
    //     }

    //     if let Err(err) = ClipboardService::set(payload) {
    //         self.event_tx.send(Event::BackendMessage(Message::new(
    //             true,
    //             format!("Error: Failed to copy to clipboard:\n\n{err}"),
    //         )))?;

    //         return Ok(());
    //     }

    //     self.event_tx.send(Event::BackendMessage(Message::new(
    //         true,
    //         "Copied to clipboard".to_string(),
    //     )))?;

    //     Ok(())
    // }
}
