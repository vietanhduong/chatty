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
    config::{BackendConfig, verbose},
    models::{ArcEventTx, BackendKind, BackendPrompt, Model},
};
use async_trait::async_trait;
use eyre::{Context, Result};
use std::{sync::Arc, time::Duration};

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
    if !config.mcp_servers.is_empty() {
        verbose!("  [+] Initializing MCP manager");
    }
    let mcp_manager = mcp::Manager::default()
        .from(&config.mcp_servers)
        .await
        .wrap_err("creating mcp manager")?;

    let avail_tools = mcp_manager.list_tools().await.wrap_err("listing tools")?;
    let mcp_manager = if !avail_tools.is_empty() {
        verbose!("  [+] MCP available {} tools", avail_tools.len(),);
        Some(Arc::new(mcp_manager))
    } else {
        None
    };

    let mut manager = manager::Manager::default();
    let default_timeout = config.timeout_secs;
    for connection in connections {
        let backend: Result<ArcBackend> = match connection.kind() {
            BackendKind::OpenAI => {
                let mut connection = connection.clone();
                if connection.timeout().is_none() && default_timeout.is_some() {
                    connection = connection
                        .with_timeout(Duration::from_secs(default_timeout.unwrap() as u64));
                }

                let mut openai: OpenAI = (&connection).into();
                if let Some(mcp_manager) = mcp_manager.as_ref() {
                    openai = openai.with_mcp(mcp_manager.clone());
                }

                openai.init().await.wrap_err("initializing OpenAI")?;

                Ok(Arc::new(openai))
            }
            BackendKind::Gemini => {
                let mut connection = connection.clone();
                if connection.timeout().is_none() && default_timeout.is_some() {
                    connection = connection
                        .with_timeout(Duration::from_secs(default_timeout.unwrap() as u64));
                }
                let mut gemini: Gemini = (&connection).into();
                if let Some(mcp_manager) = mcp_manager.as_ref() {
                    gemini = gemini.with_mcp(mcp_manager.clone());
                }

                gemini.init().await.wrap_err("initializing Gemini")?;
                Ok(Arc::new(gemini))
            }
        };

        let backend = match backend {
            Ok(backend) => backend,
            Err(e) => {
                log::warn!("  [-] Failed to initialize backend: {}", e);
                continue;
            }
        };

        let name = backend.name().to_string();
        if let Err(err) = manager.add_connection(backend).await {
            log::warn!("  [-] Failed to add backend connection: {}", err);
            continue;
        }
        verbose!("  [+] Added backend: {}", name);
        log::debug!("Added backend connection: {}", name);
    }

    if manager.is_empty() {
        eyre::bail!("No backend connections available");
    }

    Ok(Arc::new(manager))
}
