use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time;

use chatty::app::Initializer;
use chatty::app::app::InitProps;
use chatty::app::services::action::ActionService;
use chatty::app::services::{ClipboardService, EventService, ShutdownCoordinator};
use chatty::backend::new_manager;
use chatty::config::{init_logger, init_theme};
use chatty::context::Compressor;
use chatty::models::Conversation;
use chatty::models::action::Action;
use chatty::models::storage::FilterConversation;
use chatty::storage::new_storage;
use chatty::{
    app::{App, destruct_terminal},
    cli::Command,
};
use chatty::{info_notice, task_success, warn_notice};
use eyre::{Context, Result};
use tokio::{sync::mpsc, task};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<()> {
    let cmd = Command::default();
    if cmd.version() {
        cmd.print_version();
        return Ok(());
    }

    std::panic::set_hook(Box::new(|panic_info| {
        destruct_terminal();
        better_panic::Settings::auto().create_panic_handler()(panic_info);
    }));

    let init_handler = task::spawn(async move { Initializer::default().run().await });
    // Wait until the initialization screen is ready
    while !Initializer::ready() {
        tokio::time::sleep(time::Duration::from_millis(100)).await;
    }
    Initializer::add_task("init_logger", "Initializing logger..");
    let config = cmd.get_config()?;
    init_logger(&config.log)?;
    task_success!("init_logger");

    Initializer::add_task("init_theme", "Initializing theme...");
    let theme = init_theme(&config.theme)?;
    task_success!("init_theme");

    if config.backend.connections.is_empty() {
        eyre::bail!("No backend configured");
    }

    let backend = new_manager(&config.backend).await?;

    if !config.context.compression.enabled && !config.context.truncation.enabled {
        Initializer::add_notice(warn_notice!(
            "Context compression and truncation are disabled"
        ));
    }

    Initializer::add_task("listing_models", "Fetching models...");
    let models = backend.list_models().await.wrap_err("getting models")?;
    task_success!(
        "listing_models",
        format!("Available {} model(s)", models.len())
    );

    if config.context.compression.enabled {
        Initializer::add_notice(info_notice!("Context compression enabled"));
    }

    if config.context.truncation.enabled {
        Initializer::add_notice(info_notice!("Context truncation enabled"));
    }

    Initializer::add_task("init_storage", "Initializing storage...");
    let storage = new_storage(&config.storage)
        .await
        .wrap_err("initializing storage")?;
    task_success!("init_storage");

    Initializer::add_task("listing_conversations", "Fetching conversations...");
    let conversations = storage
        .get_conversations(FilterConversation::default())
        .await
        .wrap_err("getting conversations")?
        .into_iter()
        .filter(|(id, convo)| !id.is_empty() && !convo.messages().is_empty())
        .map(|(id, convo)| {
            let convo = Conversation::default()
                .with_id(&id)
                .with_created_at(convo.created_at())
                .with_updated_at(convo.updated_at())
                .with_title(convo.title());
            (id, convo)
        })
        .collect::<HashMap<_, _>>();
    task_success!(
        "listing_conversations",
        format!("Total {} conversation(s)", conversations.len())
    );

    // Mark complete tasks. We assume that all tasks are completed
    Initializer::complete();
    if let Err(err) = init_handler.await {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (action_tx, action_rx) = mpsc::unbounded_channel::<Action>();

    let mut events = EventService::default();

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
        task_set.spawn(async move { ClipboardService::start(token_clone).await });
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
