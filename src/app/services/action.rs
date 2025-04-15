use eyre::Result;
use std::sync::{Arc, atomic};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    backend::ArcBackend,
    context::Compressor,
    error_event, info_event,
    models::{
        Action, ArcEventTx, BackendPrompt, Context, Conversation, Event, Message,
        UpsertConvoRequest,
    },
    storage::ArcStorage,
    warn_event,
};

use super::clipboard::ClipboardService;

pub struct ActionService {
    backend: ArcBackend,
    storage: ArcStorage,
    compressor: Arc<Compressor>,

    action_rx: mpsc::UnboundedReceiver<Action>,
    event_tx: mpsc::UnboundedSender<Event>,
    cancel_token: CancellationToken,
    pending_tasks: Arc<atomic::AtomicUsize>,

    worker: Option<tokio::task::JoinHandle<Result<()>>>,
}

impl ActionService {
    pub fn new(
        backend: ArcBackend,
        storage: ArcStorage,
        compressor: Arc<Compressor>,

        action_rx: mpsc::UnboundedReceiver<Action>,
        event_tx: mpsc::UnboundedSender<Event>,
        cancel_token: CancellationToken,
        pending_tasks: Arc<atomic::AtomicUsize>,
    ) -> Self {
        Self {
            backend,
            storage,
            compressor,

            action_rx,
            event_tx,
            cancel_token,
            pending_tasks,
            worker: None,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        log::debug!("Action service started");
        loop {
            tokio::select! {
                biased;
                Some(action) = self.action_rx.recv() => self.process_action(action).await,
                _ = self.cancel_token.cancelled() => {
                    return Ok(());
                }
            }
        }
    }

    async fn process_action(&mut self, action: Action) {
        match action {
            Action::BackendAbort => {
                if let Some(worker) = self.worker.take() {
                    self.pending_tasks.fetch_sub(1, atomic::Ordering::SeqCst);
                    worker.abort();
                    let _ = self.event_tx.send(Event::BackendAbort);
                }
            }
            Action::BackendRequest(prompt) => {
                let backend = Arc::clone(&self.backend);
                let event_tx: ArcEventTx = Arc::new(self.event_tx.clone());

                self.pending_tasks.fetch_add(1, atomic::Ordering::SeqCst);
                self.worker = Some(tokio::spawn(async move {
                    if let Err(err) = completions(&backend, prompt, Arc::clone(&event_tx)).await {
                        worker_error(err, Arc::clone(&event_tx)).await?;
                    }
                    Ok(())
                }))
            }

            Action::CopyMessages(messages) => {
                if let Err(err) = self.copy_messages(messages).await {
                    log::error!("Failed to copy messages: {}", err);
                    let _ = self
                        .event_tx
                        .send(error_event!(format!("Failed to copy messages: {}", err)));
                }
            }

            Action::CopyText { content, notice } => {
                if let Err(err) = self.copy_text(content, notice).await {
                    log::error!("Failed to copy text: {}", err);
                    let _ = self
                        .event_tx
                        .send(error_event!(format!("Failed to copy text: {}", err)));
                }
            }

            Action::DeleteConversation(id) => self.process_delete_convo(&id).await,
            Action::UpsertConversation(req) => self.process_upsert_convo(req).await,
            Action::UpsertMessage(convo_id, message) => {
                self.process_upsert_message(&convo_id, message).await
            }
            Action::UpsertConvoContext(convo_id, ctx) => {
                self.process_upsert_context(&convo_id, ctx).await
            }
            Action::DeleteMessage(msg_id) => self.process_delete_message(&msg_id).await,
            Action::CompressConversation(convo_id, model_id) => {
                self.process_copress_convo(&convo_id, &model_id)
            }
            Action::SetConversation(convo_id) => {
                self.pending_tasks.fetch_add(1, atomic::Ordering::SeqCst);
                let result = self.get_convo(&convo_id).await;
                self.pending_tasks.fetch_sub(1, atomic::Ordering::SeqCst);
                let _ = self.event_tx.send(Event::SetConversation(result));
            }
        }
    }

    async fn process_delete_convo(&mut self, convo_id: &str) {
        self.pending_tasks.fetch_add(1, atomic::Ordering::SeqCst);
        let result = self.storage.delete_conversation(convo_id).await;
        self.pending_tasks.fetch_sub(1, atomic::Ordering::SeqCst);
        if let Err(err) = result {
            let _ = self.event_tx.send(error_event!(format!(
                "Failed to delete conversation: {}",
                err
            )));
            return;
        }
        let _ = self
            .event_tx
            .send(Event::ConversationDeleted(convo_id.to_string()));
    }

    async fn process_delete_message(&mut self, msg_id: &str) {
        self.pending_tasks.fetch_add(1, atomic::Ordering::SeqCst);
        let result = self.storage.delete_messsage(msg_id).await;
        self.pending_tasks.fetch_sub(1, atomic::Ordering::SeqCst);
        if let Err(err) = result {
            let _ = self
                .event_tx
                .send(error_event!(format!("Failed to delete message: {}", err)));
        }
    }

    async fn process_upsert_context(&mut self, convo_id: &str, ctx: Context) {
        self.pending_tasks.fetch_add(1, atomic::Ordering::SeqCst);
        let result = self.storage.upsert_context(convo_id, ctx).await;
        self.pending_tasks.fetch_sub(1, atomic::Ordering::SeqCst);
        if let Err(err) = result {
            let _ = self
                .event_tx
                .send(error_event!(format!("Failed to upsert context: {}", err)));
        }
    }

    async fn process_upsert_message(&mut self, convo_id: &str, message: Message) {
        self.pending_tasks.fetch_add(1, atomic::Ordering::SeqCst);
        let result = self.storage.upsert_message(convo_id, message).await;
        self.pending_tasks.fetch_sub(1, atomic::Ordering::SeqCst);
        if let Err(err) = result {
            let _ = self
                .event_tx
                .send(error_event!(format!("Failed to upsert message: {}", err)));
        }
    }

    async fn process_upsert_convo(&mut self, req: UpsertConvoRequest) {
        self.pending_tasks.fetch_add(1, atomic::Ordering::SeqCst);
        let convo_id = req.convo.id().to_string();
        let result = self.storage.upsert_conversation(req.convo.clone()).await;
        self.pending_tasks.fetch_sub(1, atomic::Ordering::SeqCst);
        if let Err(err) = result {
            let _ = self.event_tx.send(error_event!(format!(
                "Failed to upsert conversation: {}",
                err
            )));
            return;
        }

        if req.include_messages {
            for msg in req.convo.messages() {
                self.process_upsert_message(&convo_id, msg.clone()).await;
            }
        }

        if req.include_context {
            for ctx in req.convo.contexts() {
                self.process_upsert_context(&convo_id, ctx.clone()).await;
            }
        }

        // Fetch the conversation in database and send it back to the event channel
        // Because the input conversation might has no context or messages.
        if let Some(convo) = self.get_convo(&convo_id).await {
            let _ = self.event_tx.send(Event::ConversationUpdated(convo));
        }
    }

    async fn get_convo(&self, convo_id: &str) -> Option<Conversation> {
        let result = self.storage.get_conversation(convo_id).await;
        if let Err(err) = result {
            let _ = self
                .event_tx
                .send(error_event!(format!("Failed to get conversation: {}", err)));
            return None;
        }
        result.unwrap()
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
        let _ = self
            .event_tx
            .send(info_event!("Copied messages to clipboard!"));
        Ok(())
    }

    async fn copy_text(&self, content: String, notice: bool) -> Result<()> {
        ClipboardService::set(content)?;
        if notice {
            let _ = self.event_tx.send(info_event!("Copied text to clipboard!"));
        }
        Ok(())
    }

    fn process_copress_convo(&mut self, conversation_id: &str, model_id: &str) {
        let storage = self.storage.clone();
        let compressor = self.compressor.clone();
        let conversation_id = conversation_id.to_string();
        let model_id = model_id.to_string();
        let event_tx = self.event_tx.clone();
        self.pending_tasks.fetch_add(1, atomic::Ordering::SeqCst);

        let pending_tasks = self.pending_tasks.clone();
        tokio::spawn(async move {
            let _ = event_tx.send(warn_event!(
                format!("Compressing conversation using model \"{}\"... Please do NOT close the app until this process is finished!", model_id)
            ));

            let convo = match storage.get_conversation(&conversation_id).await {
                Ok(conversation) => conversation,
                Err(err) => {
                    log::error!("Failed to get conversation: {}", err);
                    let _ =
                        event_tx.send(warn_event!(format!("Failed to get conversation: {}", err)));
                    None
                }
            };

            if convo.is_none() {
                pending_tasks.fetch_sub(1, atomic::Ordering::SeqCst);
                return;
            }

            let convo = convo.unwrap();

            let context = match compressor.compress(&model_id, &convo).await {
                Ok(context) => context,
                Err(err) => {
                    log::error!("Failed to compress conversation: {}", err);
                    let _ = event_tx.send(warn_event!(format!(
                        "Failed to compress conversation: {}",
                        err
                    )));
                    None
                }
            };

            if context.is_none() {
                pending_tasks.fetch_sub(1, atomic::Ordering::SeqCst);
                return;
            }

            // Push the context to the conversation
            if let Err(err) = storage
                .upsert_context(&conversation_id, context.unwrap())
                .await
            {
                let _ = event_tx.send(warn_event!(format!("Failed to save context: {}", err)));
                pending_tasks.fetch_sub(1, atomic::Ordering::SeqCst);
                return;
            }

            if let Some(convo) = storage.get_conversation(&conversation_id).await.unwrap() {
                let _ = event_tx.send(Event::ConversationUpdated(convo));
                let _ = event_tx.send(info_event!("Context compressed!"));
            }
            pending_tasks.fetch_sub(1, atomic::Ordering::SeqCst);
        });
    }
}

async fn completions(
    backend: &ArcBackend,
    prompt: BackendPrompt,
    event_tx: ArcEventTx,
) -> Result<()> {
    backend.get_completion(prompt, event_tx).await?;
    Ok(())
}

async fn worker_error(err: eyre::Error, event_tx: ArcEventTx) -> Result<()> {
    event_tx
        .send(Event::BackendMessage(Message::new_system(
            "system",
            format!("Error: Backend failed with the following error: \n\n {err:?}"),
        )))
        .await?;

    Ok(())
}
