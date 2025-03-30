use std::sync::Arc;

use crate::backend::ArcBackend;
use crate::models::{Action, ArcEventTx, BackendPrompt, Event, Message, NoticeMessage, NoticeType};
use eyre::Result;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use super::ClipboardService;

pub struct ActionService<'a> {
    event_tx: ArcEventTx,
    action_rx: &'a mut mpsc::UnboundedReceiver<Action>,
    cancel_token: CancellationToken,
    backend: ArcBackend,
}

impl ActionService<'_> {
    pub fn new(
        event_tx: ArcEventTx,
        action_rx: &'_ mut mpsc::UnboundedReceiver<Action>,
        backend: ArcBackend,
        cancel_token: CancellationToken,
    ) -> ActionService<'_> {
        ActionService {
            cancel_token,
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
            tokio::select! {
                _ = self.cancel_token.cancelled() => {
                    log::debug!("Action service cancelled");
                    return Ok(());
                }

                event = self.action_rx.recv() => {
                    if event.is_none() {
                        continue;
                    }
                    let event = event.unwrap();
                    let worker_tx = Arc::clone(&self.event_tx);
                    // let storage = Arc::clone(&self.storage);
                    let backend = Arc::clone(&self.backend);
                    match event {

                        Action::CopyMessages(messages) => {
                            if let Err(err) = self.copy_messages(messages).await {
                                log::error!("Failed to copy messages: {}", err);
                                self.send_notice(
                                    NoticeType::Error,
                                    format!("Failed to copy messages: {}", err),
                                ).await;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn send_notice(&self, notice_type: NoticeType, message: impl Into<String>) {
        self.event_tx
            .send(Event::Notice(
                NoticeMessage::new(message).with_type(notice_type),
            ))
            .await
            .unwrap_or_else(|err| {
                log::error!("Failed to send notice: {}", err);
            });
    }

    async fn copy_messages(&self, messages: Vec<Message>) -> Result<()> {
        let mut payload = messages[0].text().to_string();
        if messages.len() > 1 {
            payload = messages
                .iter()
                .map(|msg| msg.text().to_string())
                .collect::<Vec<String>>()
                .join("\n\n")
        }

        ClipboardService::set(payload)?;
        self.event_tx
            .send(Event::Notice(NoticeMessage::new("Copied to clipboard!")))
            .await?;
        Ok(())
    }
}
