pub mod binary_transport;
pub mod client;

use crate::models::mcp::{CallToolResult, Tool};
use eyre::Result;
use serde_json::Value;

#[async_trait::async_trait]
pub trait MCP: Send + Sync + 'static {
    async fn list_tools(&self) -> Result<Vec<Tool>>;
    async fn call_tool(&self, tool: &str, args: Option<Value>) -> Result<CallToolResult>;
}
