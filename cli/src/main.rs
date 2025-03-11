use std::sync::Arc;

use eyre::Result;
use openai_app::{App, services::ActionService};
use openai_backend::new_boxed_backend;
use openai_models::{Action, Event};
use openai_tui::{Command, init_logger, init_theme};
use tokio::{sync::mpsc, task};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Command::get_config()?;
    init_logger(&config)?;

    let theme = init_theme(&config)?;

    let mut backend = new_boxed_backend(&config)?;
    backend.health_check().await?;

    let models = backend.list_models().await?;
    if models.is_empty() {
        eyre::bail!("No models available");
    }

    let want_models = config
        .backend()
        .cloned()
        .unwrap_or_default()
        .models()
        .unwrap_or_default()
        .to_vec();

    let model = if want_models.is_empty() {
        models[0].clone()
    } else {
        want_models
            .iter()
            .filter(|m| models.contains(m))
            .next()
            .unwrap_or(&models[0])
            .clone()
    };

    backend.set_model(&model).await?;

    let backend = Arc::new(backend);

    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();

    let mut bg_futures = task::JoinSet::new();

    bg_futures.spawn(async move {
        ActionService::new(event_tx, &mut action_rx, Arc::clone(&backend))
            .start()
            .await
    });

    let mut app = App::new(&theme, action_tx.clone(), &mut event_rx);

    app.run().await?;
    Ok(())
}
