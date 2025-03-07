use std::sync::Arc;

use eyre::Result;
use openai_app::{action::ActionService, app::start};
use openai_backend::{BoxedBackend, OpenAI};
use openai_models::{Action, Event, Message};
use tokio::{sync::mpsc, task};

#[tokio::main]
async fn main() -> Result<()> {
    let mut backend: BoxedBackend = Box::new(
        OpenAI::default()
            .with_endpoint("https://api.deepseek.com")
            .with_token(""),
    );

    backend.health_check().await?;
    backend.set_model("deepseek-chat").await?;

    let backend = Arc::new(backend);

    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();
    let (event_tx, event_rx) = mpsc::unbounded_channel::<Event>();

    event_tx.send(Event::BackendMessage(Message::new(
        true,
        format!("Current model: {}", backend.current_model()),
    )))?;

    let mut bg_futures = task::JoinSet::new();

    bg_futures.spawn(async move {
        ActionService::new(event_tx, &mut action_rx, Arc::clone(&backend))
            .start()
            .await
    });

    Ok(start(action_tx, event_rx).await?)
}
