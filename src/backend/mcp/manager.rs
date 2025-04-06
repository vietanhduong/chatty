#[cfg(test)]
#[path = "manager_test.rs"]
mod tests;

use super::{CallToolResult, Tool};
use super::{McpClient, client::Client};
use crate::config::McpServerConfig;
use eyre::{Context, Result};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub struct Manager {
    tools: HashMap<Tool, Arc<dyn McpClient>>, // Tool name - MCP Client
}

impl Manager {
    pub async fn from(mut self, servers: &[McpServerConfig]) -> Result<Self> {
        for server in servers.iter().filter(|s| s.enabled.unwrap_or(true)) {
            let client = Client::new(&server.provider, &server.server)
                .await
                .wrap_err("creating client")?;
            self.add_server(Arc::new(client)).await?;
        }
        Ok(self)
    }

    pub async fn add_server(&mut self, client: Arc<dyn McpClient>) -> Result<()> {
        client
            .list_tools()
            .await
            .wrap_err("listing tools")?
            .into_iter()
            .for_each(|tool| {
                if let Some((k, _)) = self.tools.get_key_value(&tool) {
                    let k = k.clone();
                    // If the key already exists, we will compare which one has longer
                    // description and keep the one with longest description
                    if k.description.as_deref().unwrap_or_default().len()
                        > tool.description.as_deref().unwrap_or_default().len()
                    {
                        return;
                    }
                    // Otherwise, we delete the old one and insert the new one
                    self.tools.remove(&k);
                }
                self.tools.insert(tool, client.clone());
            });
        Ok(())
    }
}

#[async_trait::async_trait]
impl McpClient for Manager {
    /// List all available tools
    async fn list_tools(&self) -> Result<Vec<Tool>> {
        // FIXME: Should we apply a TTL cache for this?
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
            .find(|(k, _)| k.name.as_str() == tool)
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
