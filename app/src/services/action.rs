use std::sync::Arc;

use eyre::Result;
use openai_backend::ArcBackend;
use openai_models::{Action, BackendPrompt, Event, Message, NoticeMessage, NoticeType};
use openai_storage::ArcStorage;
use tokio::{sync::mpsc, task::JoinHandle};

use super::ClipboardService;

pub struct ActionService<'a> {
    event_tx: mpsc::UnboundedSender<Event>,
    action_rx: &'a mut mpsc::UnboundedReceiver<Action>,
    backend: ArcBackend,
    storage: ArcStorage,
}

impl ActionService<'_> {
    pub fn new(
        event_tx: mpsc::UnboundedSender<Event>,
        action_rx: &'_ mut mpsc::UnboundedReceiver<Action>,
        backend: ArcBackend,
        storage: ArcStorage,
    ) -> ActionService<'_> {
        ActionService {
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
            let event = self.action_rx.recv().await;
            if event.is_none() {
                continue;
            }

            let worker_tx = self.event_tx.clone();
            match event.unwrap() {
                Action::RemoveMessage(id) => {
                    if let Err(err) = self.storage.delete_messsage(&id).await {
                        log::error!("Failed to delete message: {}", err);
                        self.send_notice(
                            NoticeType::Error,
                            format!("Failed to delete message: {}", err),
                        );
                        continue;
                    }
                }
                Action::UpsertMessage(request) => {
                    if let Err(err) = self
                        .storage
                        .upsert_message(&request.conversation_id, request.message)
                        .await
                    {
                        log::error!("Failed to append message: {}", err);
                        self.send_notice(
                            NoticeType::Error,
                            format!("Failed to append message: {}", err),
                        );
                        continue;
                    }
                }
                Action::UpsertConversation(conversation) => {
                    if let Err(err) = self.storage.upsert_converstation(conversation).await {
                        log::error!("Failed to upsert conversation: {}", err);
                        self.send_notice(
                            NoticeType::Error,
                            format!("Failed to upsert conversation: {}", err),
                        );
                        continue;
                    }
                }
                Action::GetConversation(con) => {
                    let conversation = match self.storage.get_conversation(&con).await {
                        Ok(conversation) => conversation,
                        Err(err) => {
                            log::error!("Failed to get conversation: {}", err);
                            self.send_notice(
                                NoticeType::Error,
                                format!("Failed to get conversation: {}", err),
                            );
                            continue;
                        }
                    };

                    match conversation {
                        Some(conversation) => {
                            worker_tx.send(Event::ConversationResponse(conversation))?;
                        }
                        None => {
                            self.send_notice(
                                NoticeType::Warning,
                                "Conversation not found".to_string(),
                            );
                        }
                    }
                }
                Action::ListConversations => {
                    let conversations =
                        match self.storage.get_conversations(Default::default()).await {
                            Ok(conversations) => conversations,
                            Err(err) => {
                                log::error!("Failed to get conversations: {}", err);
                                self.send_notice(
                                    NoticeType::Error,
                                    format!("Failed to get conversations: {}", err),
                                );
                                continue;
                            }
                        };
                    worker_tx.send(Event::ListConversationsResponse(conversations))?;
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
                        self.send_notice(
                            NoticeType::Error,
                            format!("Failed to copy messages: {}", err),
                        );
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
