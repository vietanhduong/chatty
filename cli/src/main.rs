use std::sync::Arc;

use eyre::Result;
use openai_app::{App, services::ActionService};
use openai_backend::new_backend;
use openai_models::{Action, Event};
use openai_tui::{Command, init_logger, init_theme};
use tokio::{sync::mpsc, task};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Command::get_config()?;
    init_logger(&config)?;

    let theme = init_theme(&config)?;

    let backend = new_backend(&config)?;
    {
        let mut lock = backend.lock().await;
        lock.health_check().await?;

        let models = lock.list_models().await?;
        if models.is_empty() {
            eyre::bail!("No models available");
        }

        let backend_config = config.backend().cloned().unwrap_or_default();
        let want_model = backend_config.default_model().unwrap_or_default();

        let model = if want_model.is_empty() {
            models[0].clone()
        } else {
            models
                .iter()
                .find(|m| m == &&want_model)
                .unwrap_or_else(|| {
                    log::warn!(
                        "Model {} not found, using default ({})",
                        want_model,
                        models[0]
                    );
                    &models[0]
                })
                .clone()
        };

        lock.set_model(&model).await?;
    }

    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();

    let mut bg_futures = task::JoinSet::new();

    let mut app = App::new(
        Arc::clone(&backend),
        &theme,
        action_tx.clone(),
        &mut event_rx,
    );

    bg_futures.spawn(async move {
        ActionService::new(event_tx, &mut action_rx, backend)
            .start()
            .await
    });

    app.run().await?;
    Ok(())
}
