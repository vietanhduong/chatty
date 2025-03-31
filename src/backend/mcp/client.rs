use mcp_rust_sdk::transport::stdio::StdioTransport;

pub struct Client {
    inner: mcp_rust_sdk::client::Client,
}
