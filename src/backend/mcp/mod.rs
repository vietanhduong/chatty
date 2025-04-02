pub mod client;
pub mod manager;
pub mod models;
mod transport;

pub use client::Client;
pub use manager::Manager;
pub use models::*;

use eyre::Result;
use serde_json::Value;

#[async_trait::async_trait]
pub trait MCP: Send + Sync + 'static {
    async fn list_tools(&self) -> Result<Vec<Tool>>;
    async fn call_tool(&self, tool: &str, args: Option<Value>) -> Result<CallToolResult>;
    async fn shutdown(&self) -> Result<()>;
}
