use std::{collections::HashMap, sync::Arc};

use crate::models::mcp::{CallToolResult, Tool};

use super::{MCP, binary_transport::BinaryTransportBuilder};
use eyre::{Context, Result};
use futures::StreamExt;
use mcp_rust_sdk::{
    Request,
    protocol::RequestId,
    transport::{Message, Transport},
};

pub struct Client {
    transport: Arc<dyn Transport>,
    request_counter: Arc<tokio::sync::RwLock<i64>>,
}

impl Client {
    pub fn new_binary(builder: BinaryTransportBuilder) -> Result<Self> {
        let transport = builder.build().wrap_err("initializing binary transport")?;
        Ok(Self {
            transport: Arc::new(transport),
            request_counter: Arc::new(tokio::sync::RwLock::new(0)),
        })
    }

    /// Send a request to the server and wait for the response.
    ///
    /// This method will block until a response is received from the server.
    /// If the server returns an error, it will be propagated as an `Error`.
    pub async fn request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let mut counter = self.request_counter.write().await;
        *counter += 1;
        let id = RequestId::Number(*counter);

        let request = Request::new(method, params, id.clone());
        self.transport.send(Message::Request(request)).await?;

        let mut stream = self.transport.receive();
        // Wait for matching response
        while let Some(resp) = stream.next().await {
            match resp {
                Ok(Message::Response(resp)) => {
                    if resp.id != id {
                        return Err(eyre::eyre!("response id mismatch"));
                    }
                    if resp.error.is_some() {
                        return Err(eyre::eyre!("server error: {:?}", resp.error));
                    }
                    return Ok(resp.result.unwrap_or_default());
                }
                Ok(_) => continue,
                Err(e) => return Err(e.into()),
            }
        }
        eyre::bail!("no response received")
    }
}

#[async_trait::async_trait]
impl MCP for Client {
    /// List all available tools
    async fn list_tools(&self) -> Result<Vec<Tool>> {
        let resp = self
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
        self.transport
            .close()
            .await
            .wrap_err("shutting down client")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_client() {
        let builder = BinaryTransportBuilder::new("hyper-mcp");
        let client = Client::new_binary(builder).unwrap();
        let resp = client.call_tool("myip", None).await.unwrap();
        println!(
            "Response: {:?}",
            serde_json::to_string(&resp.content).unwrap()
        );
    }
}
