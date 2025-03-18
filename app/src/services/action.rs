use std::sync::Arc;

use eyre::Result;
use openai_backend::ArcBackend;
use openai_models::{Action, BackendPrompt, Event, Message, NoticeMessage, NoticeType};
use openai_storage::ArcStorage;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use super::ClipboardService;

pub struct ActionService<'a> {
    event_tx: mpsc::UnboundedSender<Event>,
    action_rx: &'a mut mpsc::UnboundedReceiver<Action>,
    cancel_token: CancellationToken,
    backend: ArcBackend,
    storage: ArcStorage,
}

impl ActionService<'_> {
    pub fn new(
        event_tx: mpsc::UnboundedSender<Event>,
        action_rx: &'_ mut mpsc::UnboundedReceiver<Action>,
        backend: ArcBackend,
        storage: ArcStorage,
        cancel_token: CancellationToken,
    ) -> ActionService<'_> {
        ActionService {
            cancel_token,
            event_tx,
            action_rx,
            backend,
            storage,
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
                    let worker_tx = self.event_tx.clone();
                    let storage = Arc::clone(&self.storage);
                    let backend = Arc::clone(&self.backend);
                    match event {
                        Action::RemoveMessage(id) => {
                            if let Err(err) = storage.delete_messsage(&id).await {
                                log::error!("Failed to delete message: {}", err);
                                send_notice(
                                    worker_tx,
                                    NoticeType::Error,
                                    format!("Failed to delete message: {}", err),
                                );
                                return Err(err);
                            }
                        }

                        Action::UpsertMessage(request) => {
                            if let Err(err) = storage
                                .upsert_message(&request.conversation_id, request.message)
                                .await
                            {
                                log::error!("Failed to append message: {}", err);
                                send_notice(
                                    worker_tx,
                                    NoticeType::Error,
                                    format!("Failed to append message: {}", err),
                                );
                                return Err(err);
                            }
                            log::debug!("Upserted message");
                        }

                        Action::RemoveConversation(id) => {
                            if let Err(err) = storage.delete_conversation(&id).await {
                                log::error!("Failed to delete conversation: {}", err);
                                send_notice(
                                    worker_tx,
                                    NoticeType::Error,
                                    format!("Failed to delete conversation: {}", err),
                                );
                                return Err(err);
                            }
                            worker_tx.send(Event::ConversationDeleted(id))?;
                            log::debug!("Deleted conversation");
                        }

                        Action::UpsertConversation(conversation) => {
                            if let Err(err) = storage.upsert_conversation(conversation).await {
                                log::error!("Failed to upsert conversation: {}", err);
                                send_notice(
                                    worker_tx,
                                    NoticeType::Error,
                                    format!("Failed to upsert conversation: {}", err),
                                );
                                return Err(err);
                            }
                            log::debug!("Upserted conversation");
                        }

                        Action::BackendSetModel(model) => {
                            if let Err(err) = self.backend.set_default_model(&model).await {
                                log::error!("Failed to set model: {}", err);
                                self.send_notice(
                                    NoticeType::Error,
                                    format!("Failed to set model: {}", err),
                                );
                                continue;
                            }
                            worker_tx.send(Event::ModelChanged(model))?;
                        }

                        Action::BackendAbort => {
                            worker.abort();
                            worker_tx.send(Event::AbortRequest)?;
                        }

                        Action::BackendRequest(prompt) => {
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
                                self.send_notice(
                                    NoticeType::Error,
                                    format!("Failed to copy messages: {}", err),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn send_notice(&self, notice_type: NoticeType, message: impl Into<String>) {
        self.event_tx
            .send(Event::Notice(
                NoticeMessage::new(message).with_type(notice_type),
            ))
            .unwrap_or_else(|err| {
                log::error!("Failed to send notice: {}", err);
            });
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

        ClipboardService::set(payload)?;
        self.event_tx
            .send(Event::Notice(NoticeMessage::new("Copied to clipboard!")))?;
        Ok(())
    }
}

async fn completions(
    backend: &ArcBackend,
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

fn send_notice(
    event_tx: mpsc::UnboundedSender<Event>,
    notice_type: NoticeType,
    message: impl Into<String>,
) {
    event_tx
        .send(Event::Notice(
            NoticeMessage::new(message).with_type(notice_type),
        ))
        .unwrap_or_else(|err| {
            log::error!("Failed to send notice: {}", err);
        });
}
