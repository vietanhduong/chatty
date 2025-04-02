use std::{collections::HashMap, sync::Arc};

use crate::{
    config::{BinaryConfig, MCPConfig, WebSocketConfig},
    models::mcp::{CallToolResult, Tool},
};

use super::{MCP, transport::Binary};
use eyre::{Context, Result};
use mcp_rust_sdk::transport::websocket::WebSocketTransport;

pub struct Client {
    inner: mcp_rust_sdk::client::Client,
}

impl Client {
    pub fn new_binary(config: &BinaryConfig) -> Result<Self> {
        let transport = Arc::new(Binary::new(config).wrap_err("initializing binary transport")?);
        let inner = mcp_rust_sdk::client::Client::new(transport);
        Ok(Self { inner })
    }

    pub async fn new_websocket(config: &WebSocketConfig) -> Result<Self> {
        let transport = Arc::new(WebSocketTransport::new(&config.url).await?);
        let inner = mcp_rust_sdk::client::Client::new(transport);
        Ok(Self { inner })
    }

    pub async fn new(config: &MCPConfig) -> Result<Self> {
        match config {
            MCPConfig::Binary(binary) => Self::new_binary(binary),
            MCPConfig::WebSocket(websocket) => Self::new_websocket(websocket).await,
        }
    }
}

#[async_trait::async_trait]
impl MCP for Client {
    /// List all available tools
    async fn list_tools(&self) -> Result<Vec<Tool>> {
        let resp = self
            .inner
            .request("tools/list", None)
            .await
            .wrap_err("requesting tools")?;
        // {"tools": [...]}
        let mut resp: HashMap<String, Vec<Tool>> =
            serde_json::from_value(resp).wrap_err("parsing response")?;
        let tools = resp
            .remove("tools")
            .ok_or_else(|| eyre::eyre!("missing tools in response"))?;
        Ok(tools)
    }

    /// Call a tool with the given name and arguments
    async fn call_tool(
        &self,
        tool: &str,
        args: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        let resp = self
            .inner
            .request(
                "tools/call",
                Some(serde_json::json!({ "name": tool, "arguments": args })),
            )
            .await
            .wrap_err("requesting tool call")?;
        let result: CallToolResult = serde_json::from_value(resp).wrap_err("parsing response")?;
        Ok(result)
    }

    async fn shutdown(&self) -> Result<()> {
        self.inner.shutdown().await.wrap_err("shutting down client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_client() {
        let config = BinaryConfig {
            filename: "hyper-mcp".to_string(),
            args: vec![],
            env: HashMap::new(),
        };
        let client = Client::new_binary(&config).unwrap();
        let resp = client.call_tool("myip", None).await.unwrap();
        println!(
            "Response: {:?}",
            serde_json::to_string(&resp.content).unwrap()
        );
    }
}
