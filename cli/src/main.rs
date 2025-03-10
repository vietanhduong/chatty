use std::{fs::File, io::Write, sync::Arc};

use chrono::Local;
use env_logger::Builder;
use eyre::Result;
use log::LevelFilter;
use openai_app::{App, services::ActionService};
use openai_backend::{BoxedBackend, OpenAI};
use openai_models::{Action, Event};
use tokio::{sync::mpsc, task};

#[tokio::main]
async fn main() -> Result<()> {
    let log_file = Box::new(File::create("/tmp/openai_app.log")?);

    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{}:{} {} [{}] - {}",
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
                record.level(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(log_file))
        .filter(None, LevelFilter::Debug)
        .init();

    let mut backend: BoxedBackend = Box::new(
        OpenAI::default()
            .with_endpoint("https://api.deepseek.com")
            .with_token(std::env::var("OPENAI_API_KEY")?.as_str()),
    );

    backend.health_check().await?;
    backend.set_model("deepseek-chat").await?;

    let backend = Arc::new(backend);

    let (action_tx, mut action_rx) = mpsc::unbounded_channel::<Action>();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();

    let mut bg_futures = task::JoinSet::new();

    bg_futures.spawn(async move {
        ActionService::new(event_tx, &mut action_rx, Arc::clone(&backend))
            .start()
            .await
    });

    let mut app = App::new(action_tx.clone(), &mut event_rx);

    app.run().await?;
    Ok(())
}
