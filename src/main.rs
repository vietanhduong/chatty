use std::sync::Arc;

use chatty::backend::new_manager;
use chatty::config::{init_logger, init_theme};
use chatty::context::Compressor;
use chatty::models::{Action, ArcEventTx, Event, storage::FilterConversation};
use chatty::storage::new_storage;
use chatty::{
    app::{
        App,
        app::AppInitProps,
        destruct_terminal_for_panic,
        services::{ActionService, ClipboardService},
    },
    cli::Command,
};
use eyre::{Context, Result};
use tokio::{sync::mpsc, task};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    let cmd = Command::new();
    if cmd.version() {
        cmd.print_version();
        return Ok(());
    }

    std::panic::set_hook(Box::new(|panic_info| {
        destruct_terminal_for_panic();
        better_panic::Settings::auto().create_panic_handler()(panic_info);
    }));

    let config = cmd.get_config()?;
    init_logger(&config.log)?;
    println!("[+] Logger initialized");

    let theme = init_theme(&config.theme)?;
    println!("[+] Theme initialized");

    if config.backend.connections.is_empty() {
        eyre::bail!("No backend configured");
    }

    println!("[+] Initializing backend...");
    let backend = new_manager(&config.backend).await?;
    backend.health_check().await?;
    println!("[+] Backend is healthy");

    println!("[+] Listing models...");
    let models = backend.list_models(false).await?;
    if models.is_empty() {
        eyre::bail!("No models available");
    }
    println!("[+] Loaded {} model(s)", models.len());

    let want_model = config.backend.default_model.as_deref().unwrap_or_default();

    let model = if want_model.is_empty() {
        models[0].clone()
    } else {
        models
            .iter()
            .find(|m| m.id() == want_model)
            .unwrap_or_else(|| {
                log::warn!(
                    "Model {} not found, using default ({})",
                    want_model,
                    models[0].id()
                );
                &models[0]
            })
            .clone()
    };

    backend.set_current_model(model.id()).await?;
    println!("[+] Set current model to {}", model);

    println!("[+] Initializing storage...");
    let storage = new_storage(&config.storage)
        .await
        .wrap_err("initializing storage")?;
    println!("[+] Storage initialized");

    println!("[+] Fetching conversations...");
    let conversations = storage
        .get_conversations(FilterConversation::default())
        .await?;
    println!("[+] Conversations fetched");

    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();

    let mut bg_futures = task::JoinSet::new();

    let ctx_compress_config = &config.context.compression;

    let mut app = App::new(
        &theme,
        action_tx.clone(),
        event_tx.clone(),
        &mut event_rx,
        Arc::new(
            Compressor::new(backend.clone())
                .with_context_length(ctx_compress_config.max_tokens)
                .with_conversation_length(ctx_compress_config.max_messages)
                .with_keep_n_messages(ctx_compress_config.keep_n_messages)
                .with_enabled(ctx_compress_config.enabled),
        ),
        storage,
        AppInitProps {
            default_model: model.id().to_string(),
            models,
            conversations,
        },
    );

    let token = CancellationToken::new();

    let token_clone = token.clone();
    let event_sender: ArcEventTx = Arc::new(event_tx);
    bg_futures.spawn(async move {
        ActionService::new(event_sender, &mut action_rx, backend, token_clone)
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
