pub mod gemini;
pub mod manager;
pub mod mcp;
pub mod openai;
pub(crate) mod utils;

pub use gemini::Gemini;
pub use manager::Manager;
pub use mcp::McpClient;
pub use openai::OpenAI;

#[cfg(test)]
use mockall::{automock, predicate::*};

use crate::{
    app::Initializer,
    config::BackendConfig,
    models::{ArcEventTx, BackendConnection, BackendKind, BackendPrompt, Model},
    task_failure, task_success, warn_notice,
};
use async_trait::async_trait;
use eyre::{Context, Result};
use std::sync::Arc;

const TITLE_PROMPT: &str = r#"

---
This is initial message. Please give name a title for this conversation.
The title should be placed at the top of the response, in separate line and starts with #"#;

#[async_trait]
#[cfg_attr(test, automock)]
pub trait Backend {
    fn name(&self) -> &str;
    async fn list_models(&self) -> Result<Vec<Model>>;
    async fn get_completion(&self, prompt: BackendPrompt, event_tx: ArcEventTx) -> Result<()>;
}

pub type ArcBackend = Arc<dyn Backend + Send + Sync>;

pub async fn new_manager(config: &BackendConfig) -> Result<ArcBackend> {
    let connections = config
        .connections
        .iter()
        .filter(|c| c.enabled())
        .collect::<Vec<_>>();
    if connections.is_empty() {
        eyre::bail!("No backend connections configured");
    }

    // Init MCP manager
    if !config.mcp.servers.is_empty() {
        Initializer::add_task("init_mcp", "Initializing MCP manager...");
    }
    let mcp_manager = mcp::Manager::default()
        .from(&config.mcp.servers)
        .await
        .wrap_err("creating mcp manager")?;
    task_success!("init_mcp");

    Initializer::add_task("listing_mcp_tool", "Listing MCP Tools...");
    let avail_tools = mcp_manager.list_tools().await.wrap_err("listing tools")?;
    let mcp_manager: Option<Arc<dyn McpClient>> = if !avail_tools.is_empty() {
        Some(Arc::new(mcp_manager))
    } else {
        None
    };

    task_success!(
        "listing_mcp_tool",
        format!("Available {} tool(s)", avail_tools.len())
    );

    let mut manager = manager::Manager::default();
    for connection in connections {
        let backend = match new_backend(connection, mcp_manager.clone()).await {
            Ok(backend) => backend,
            Err(e) => {
                Initializer::add_notice(warn_notice!(format!(
                    "Failed to initialize backend: {}",
                    e
                )));
                log::warn!("Failed to initialize backend: {}", e);
                continue;
            }
        };

        Initializer::add_task(
            format!("setup_backend_{}", backend.name()).as_str(),
            format!("Setting up backend connection {}...", backend.name()).as_str(),
        );
        let name = backend.name().to_string();
        if let Err(err) = manager.add_connection(backend).await {
            task_failure!(
                format!("setup_backend_{}", name).as_str(),
                format!("Failed: {}", err)
            );
            log::warn!("Failed to add backend connection: {}", err);
            continue;
        }
        task_success!(format!("setup_backend_{}", name).as_str());
        log::debug!("Added backend connection: {}", name);
    }

    if manager.is_empty() {
        eyre::bail!("No backend connections available");
    }

    Ok(Arc::new(manager))
}

async fn new_backend(
    conn: &BackendConnection,
    mcp: Option<Arc<dyn McpClient>>,
) -> Result<ArcBackend> {
    match conn.kind() {
        BackendKind::OpenAI => {
            let mut openai: OpenAI = conn.into();
            if let Some(mcp) = mcp {
                openai = openai.with_mcp(mcp);
            }
            openai.init().await.wrap_err("initializing OpenAI")?;
            Ok(Arc::new(openai))
        }
        BackendKind::Gemini => {
            let mut gemini: Gemini = conn.into();
            if let Some(mcp) = mcp {
                gemini = gemini.with_mcp(mcp);
            }
            gemini.init().await.wrap_err("initializing Gemini")?;
            Ok(Arc::new(gemini))
        }
    }
}
