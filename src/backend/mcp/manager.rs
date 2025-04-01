use std::{collections::HashMap, sync::Arc};

use eyre::{Context, Result};

use crate::{
    config::MCPConfig,
    models::{CallToolResult, Tool},
};

use super::{MCP, client::Client};

pub struct Manager {
    tools: HashMap<Tool, Arc<dyn MCP>>, // Tool name - MCP Client
}

impl Manager {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub async fn from(mut self, servers: &[MCPConfig]) -> Result<Self> {
        for server in servers {
            let client = match server {
                MCPConfig::Binary(binary) => {
                    Client::new_binary(binary.into()).wrap_err("creating binary transport")?
                }
            };
            self.add_connection(Arc::new(client)).await?;
        }
        Ok(self)
    }

    pub async fn add_connection(&mut self, client: Arc<dyn MCP>) -> Result<()> {
        client
            .list_tools()
            .await
            .wrap_err("listing tools")?
            .into_iter()
            .for_each(|tool| {
                self.tools.insert(tool, client.clone());
            });
        Ok(())
    }
}

#[async_trait::async_trait]
impl MCP for Manager {
    /// List all available tools
    async fn list_tools(&self) -> Result<Vec<Tool>> {
        Ok(self.tools.keys().cloned().collect::<Vec<_>>())
    }

    /// Call a tool with the given name and arguments
    async fn call_tool(
        &self,
        tool: &str,
        args: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        let client = self
            .tools
            .iter()
            .filter(|(k, _)| k.name.as_str() == tool)
            .next()
            .ok_or_else(|| eyre::eyre!("tool {} not found", tool))?
            .1
            .clone();
        Ok(client.call_tool(tool, args).await?)
    }

    async fn shutdown(&self) -> Result<()> {
        for client in self.tools.values() {
            if let Err(e) = client.shutdown().await {
                log::error!("Error shutting down client: {}", e);
            }
        }
        Ok(())
    }
}
