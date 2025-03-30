use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time;

use chatty::app::app::InitProps;
use chatty::app::services::action::ActionService;
use chatty::app::services::{ClipboardService, EventService, ShutdownCoordinator};
use chatty::backend::new_manager;
use chatty::config::verbose;
use chatty::config::{init_logger, init_theme};
use chatty::context::Compressor;
use chatty::models::Conversation;
use chatty::models::action::Action;
use chatty::models::storage::FilterConversation;
use chatty::storage::new_storage;
use chatty::{
    app::{App, destruct_terminal_for_panic},
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
    verbose!("[+] Logger initialized");

    let theme = init_theme(&config.theme)?;
    verbose!("[+] Theme initialized");

    if config.backend.connections.is_empty() {
        eyre::bail!("No backend configured");
    }

    verbose!("[+] Initializing backend...");
    let backend = new_manager(&config.backend).await?;

    if !config.context.compression.enabled && !config.context.truncation.enabled {
        verbose!("[!] Context compression and truncation are disabled");
    }

    if config.context.compression.enabled {
        verbose!("[+] Context compression enabled");
    }

    if config.context.truncation.enabled {
        verbose!("[+] Context truncation enabled");
    }

    verbose!("[+] Initializing storage...");
    let storage = new_storage(&config.storage)
        .await
        .wrap_err("initializing storage")?;
    verbose!("[+] Storage initialized");

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (action_tx, action_rx) = mpsc::unbounded_channel::<Action>();

    let mut events = EventService::new();

    let mut task_set = task::JoinSet::new();
    let token = CancellationToken::new();
    let pending_tasks = Arc::new(AtomicUsize::new(0));

    let compressor =
        Arc::new(Compressor::new(backend.clone()).from_config(&config.context.compression));

    let mut action_service = ActionService::new(
        backend.clone(),
        storage.clone(),
        Arc::clone(&compressor),
        action_rx,
        events.event_tx(),
        token.clone(),
        pending_tasks.clone(),
    );

    task_set.spawn(async move { return action_service.run().await });

    verbose!("[+] Fetching models...");
    let models = backend.list_models().await.wrap_err("getting models")?;
    verbose!("[+] Fetched {} models", models.len());

    verbose!("[+] Fetching conversations...");
    let conversations = storage
        .get_conversations(FilterConversation::default())
        .await
        .wrap_err("getting conversations")?
        .into_iter()
        .map(|(id, convo)| {
            let convo = Conversation::default()
                .with_id(&id)
                .with_created_at(convo.created_at())
                .with_updated_at(convo.updated_at())
                .with_title(convo.title());
            (id, convo)
        })
        .collect::<HashMap<_, _>>();

    let mut app = App::new(
        theme,
        action_tx,
        &mut events,
        Arc::clone(&compressor),
        token.clone(),
        InitProps {
            conversations,
            models,
        },
    );

    if let Err(err) = ClipboardService::init() {
        log::warn!("Clipboard service is not available: {err}");
    } else {
        let token_clone = token.clone();
        task_set.spawn(async move {
            return ClipboardService::start(token_clone).await;
        });
    }

    let coordinator = ShutdownCoordinator {
        pending_tasks: pending_tasks.clone(),
        shutdown_complete: shutdown_tx,
        timeout: None,
    };

    task_set.spawn(coordinator.wait_for_completion());

    if let Err(err) = app.run().await {
        eprintln!("Error: {}", err);
    }

    match tokio::time::timeout(time::Duration::from_secs(15), shutdown_rx).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => eprintln!("Shutdown error: {}", e),
        Err(_) => eprintln!("Shutdown timeout reached"),
    }

    task_set.abort_all();
    while let Some(res) = task_set.join_next().await {
        match res {
            Ok(_) => {}
            Err(err) => log::error!("Task error: {}", err),
        }
    }

    Ok(())
}
