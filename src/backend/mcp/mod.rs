pub mod binary_transport;
pub mod client;

use crate::models::mcp::Tool;
use eyre::Result;

#[async_trait::async_trait]
pub trait MCP: Send + Sync + 'static {
    async fn list_tools(&self) -> Result<Vec<Tool>>;
}
