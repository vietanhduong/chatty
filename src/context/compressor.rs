#[cfg(test)]
#[path = "compressor_test.rs"]
mod tests;

use crate::backend::ArcBackend;
use crate::config::ContextCompression;
use crate::config::constants::{KEEP_N_MEESAGES, MAX_CONTEXT_LENGTH, MAX_CONVO_LENGTH};
use crate::models::{
    ArcEventTx, BackendPrompt, Context as ConvoContext, Conversation, Event, Message,
};
use eyre::{Context, Result, bail};
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct Compressor {
    enabled: bool,

    max_context_length: usize,
    max_convo_length: usize,
    keep_n_messages: usize,

    backend: ArcBackend,
}

impl Compressor {
    pub fn new(backend: ArcBackend) -> Self {
        Self {
            enabled: false,
            backend,
            max_context_length: MAX_CONTEXT_LENGTH,
            max_convo_length: MAX_CONVO_LENGTH,
            keep_n_messages: KEEP_N_MEESAGES,
        }
    }

    pub fn from_config(mut self, cfg: &ContextCompression) -> Self {
        self.enabled = cfg.enabled;
        self.max_context_length = cfg.max_tokens;
        self.max_convo_length = cfg.max_messages;
        self.keep_n_messages = cfg.keep_n_messages.max(KEEP_N_MEESAGES);
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_context_length(mut self, length: usize) -> Self {
        self.max_context_length = length;
        self
    }

    pub fn with_keep_n_messages(mut self, size: usize) -> Self {
        self.keep_n_messages = size.max(KEEP_N_MEESAGES);
        self
    }

    pub fn with_conversation_length(mut self, length: usize) -> Self {
        self.max_convo_length = length;
        self
    }

    pub fn should_compress(&self, conversation: &Conversation) -> bool {
        if !self.enabled || conversation.len() < self.keep_n_messages {
            return false;
        }

        let total_tokens = conversation.token_count();
        // Calculate the offset of the message, we will ignore the last 2
        // messages (1 user and 1 system) in-case user is asking for
        // regeneration response.
        let offset = match conversation.last_message() {
            Some(msg) => {
                if msg.is_system() {
                    2
                } else {
                    1
                }
            }
            _ => 0,
        };
        let message_count = conversation.messages().len() - offset;
        total_tokens > self.max_context_length || message_count > self.max_convo_length
    }

    pub async fn compress(
        &self,
        model: &str,
        convo: &Conversation,
    ) -> Result<Option<ConvoContext>> {
        if !self.should_compress(convo) {
            return Ok(None);
        }

        let end_checkpoint = match find_checkpoint(convo, self.keep_n_messages) {
            Some(checkpoint) => checkpoint,
            _ => return Ok(None),
        };

        let start_checkpoint = match convo.contexts().last() {
            Some(ctx) => match convo
                .messages()
                .iter()
                .position(|msg| msg.id() == ctx.last_message_id())
            {
                Some(index) => index + 1,
                _ => 0,
            },
            _ => 0,
        };

        let last_message_id = convo.messages()[end_checkpoint].id();
        let mut messages = convo
            .contexts()
            .iter()
            .map(Message::from)
            .collect::<Vec<_>>();

        messages.extend(convo.messages()[start_checkpoint..end_checkpoint + 1].to_vec());

        let message = messages
            .iter()
            .map(|msg| format!("{}: {}", message_categorize(msg), msg.text()))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = BackendPrompt::new(format!(
            r#"Summarize the following conversation in a compact yet comprehensive manner.
Focus on the key points, decisions, and any critical information exchanged, while omitting trivial or redundant details. Include specific actions or plans that were agreed upon.
Ensure that the summary is understandable on its own, providing enough context for someone who hasn't read the entire conversation.
Aim to capture the essence of the discussion while keeping the summary as concise as possible.
The summary should be started with Summary: and end with a period.
---
{}"#,
            message
        )).with_model(model).with_no_generate_title();

        let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
        let sender: ArcEventTx = Arc::new(tx);
        self.backend
            .get_completion(prompt, sender)
            .await
            .wrap_err("getting completion")?;

        let mut context = ConvoContext::new(last_message_id);
        while let Some(event) = rx.recv().await {
            match event {
                Event::BackendPromptResponse(msg) => {
                    context.append_content(msg.text);
                    if msg.done {
                        context = context.with_id(msg.id);
                        if let Some(usage) = msg.usage {
                            context = context.with_token_count(usage.completion_tokens);
                        }
                        break;
                    }
                }
                _ => bail!("Unexpected event: {:?}", event),
            }
        }

        if context.content().is_empty() {
            return Ok(None);
        }

        Ok(Some(context))
    }
}

fn find_checkpoint(conversation: &Conversation, keep_n_messages: usize) -> Option<usize> {
    let mut last = conversation.len() - 1 - keep_n_messages;
    while last > 0 && !conversation.messages()[last].is_system() {
        last -= 1;
    }
    if last == 0 {
        return None;
    }
    Some(last)
}

fn message_categorize(message: &Message) -> String {
    if message.is_context() {
        "Context".to_string()
    } else if message.is_system() {
        "System".to_string()
    } else {
        "User".to_string()
    }
}
