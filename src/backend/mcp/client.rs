use std::{collections::HashMap, sync::Arc};

use crate::models::mcp::{CallToolResult, Tool};

use super::{MCP, binary_transport::BinaryTransportBuilder};
use eyre::{Context, Result};

pub struct Client {
    inner: mcp_rust_sdk::client::Client,
}

impl Client {
    pub fn new_binary(builder: BinaryTransportBuilder) -> Result<Self> {
        let transport = builder.build().wrap_err("initializing binary transport")?;
        let client = mcp_rust_sdk::client::Client::new(Arc::new(transport));
        Ok(Self { inner: client })
    }
}

#[async_trait::async_trait]
impl MCP for Client {
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
}

#[cfg(test)]
mod tests {

    use super::*;
    #[tokio::test]
    async fn test_client() {
        let client = Client::new_binary(BinaryTransportBuilder::new("hyper-mcp")).unwrap();
        let tools = client.list_tools().await.unwrap();
        assert!(!tools.is_empty());
        println!("tools: {:?}", tools);
        let result = client.call_tool("myip", None).await.unwrap();
        println!("result: {:?}", result);
        let tools = client.list_tools().await.unwrap();
        assert!(!tools.is_empty());
        println!("tools: {:?}", tools);
    }
}
