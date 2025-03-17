use eyre::{Context, Result};
use openai_app::{
    App,
    app::AppInitProps,
    services::{ActionService, ClipboardService},
};
use openai_backend::new_backend;
use openai_models::{Action, Event, storage::FilterConversation};
use openai_storage::new_storage;
use openai_tui::{Command, init_logger, init_theme};
use tokio::{sync::mpsc, task};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Command::get_config()?;
    init_logger(&config)?;

    let theme = init_theme(&config)?;

    let backend = new_backend(&config)?;
    backend.health_check().await?;

    let models = backend.list_models(false).await?;
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

    backend.set_default_model(&model).await?;

    let storage = new_storage(&config)
        .await
        .wrap_err("initializing storage")?;

    let conversations = storage
        .get_conversations(FilterConversation::default())
        .await?;

    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();

    let mut bg_futures = task::JoinSet::new();

    if let Err(err) = ClipboardService::healthcheck() {
        log::warn!("Clipboard service is not available: {err}");
    } else {
        bg_futures.spawn(async move {
            return ClipboardService::start().await;
        });
    }

    let mut app = App::new(
        &theme,
        action_tx.clone(),
        &mut event_rx,
        AppInitProps {
            default_model: model,
            models,
            conversations,
        },
    );

    bg_futures.spawn(async move {
        ActionService::new(event_tx, &mut action_rx, backend, storage)
            .start()
            .await
    });

    app.run().await?;
    Ok(())
}
