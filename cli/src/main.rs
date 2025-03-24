use std::sync::Arc;

use eyre::{Context, Result};
use openai_app::{
    App,
    app::AppInitProps,
    destruct_terminal_for_panic,
    services::{ActionService, ClipboardService},
};
use openai_backend::new_manager;
use openai_models::{Action, ArcEventTx, Event, storage::FilterConversation};
use openai_storage::new_storage;
use openai_tui::{Command, init_logger, init_theme};
use tokio::{sync::mpsc, task};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    std::panic::set_hook(Box::new(|panic_info| {
        destruct_terminal_for_panic();
        better_panic::Settings::auto().create_panic_handler()(panic_info);
    }));

    let config = Command::get_config()?;
    init_logger(&config)?;

    let theme = init_theme(&config)?;

    if config.backend().is_none() {
        eyre::bail!("No backend configured");
    }

    let backend = new_manager(config.backend().unwrap()).await?;
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
            .find(|m| m.as_str() == want_model)
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

    backend.set_current_model(&model).await?;

    let storage = new_storage(&config)
        .await
        .wrap_err("initializing storage")?;

    let conversations = storage
        .get_conversations(FilterConversation::default())
        .await?;

    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();

    let mut bg_futures = task::JoinSet::new();

    let mut app = App::new(
        &theme,
        action_tx.clone(),
        event_tx.clone(),
        &mut event_rx,
        AppInitProps {
            default_model: model,
            models,
            conversations,
        },
    );

    let token = CancellationToken::new();

    let token_clone = token.clone();
    let event_sender: ArcEventTx = Arc::new(event_tx);
    bg_futures.spawn(async move {
        ActionService::new(event_sender, &mut action_rx, backend, storage, token_clone)
            .start()
            .await
    });

    if let Err(err) = ClipboardService::healthcheck() {
        log::warn!("Clipboard service is not available: {err}");
    } else {
        let token_clone = token.clone();
        bg_futures.spawn(async move {
            return ClipboardService::start(token_clone).await;
        });
    }

    let res = app.run().await;

    token.cancel();

    while let Some(res) = bg_futures.join_next().await {
        match res {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => {
                log::error!("Background task failed: {err}");
            }
            Err(err) => {
                log::error!("Background task panicked: {err}");
            }
        }
    }

    if res.is_err() {
        // destruct_terminal_for_panic();
        return Err(res.unwrap_err());
    }
    Ok(())
}
