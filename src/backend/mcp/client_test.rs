use mcp_rust_sdk::protocol::RequestId;
use serde_json::json;

use crate::backend::mcp::CallToolResultContent;

use super::*;
use std::time::Duration;

#[tokio::test]
async fn test_client_request_timeout() {
    // Create a mock transport with 6 second delay
    let client =
        Client::new_with_transport(Arc::new(Binary::mock("{}", Some(Duration::from_secs(2)))));

    // Try to send request with 5 second timeout
    let result = tokio::time::timeout(Duration::from_secs(1), client.list_tools()).await;

    // Should timeout
    assert!(result.is_err(), "Expected timeout error");
}

#[tokio::test]
async fn test_client_list_tools_success() {
    let tools = json!({
        "tools": [
            {
                "name": "test_tool",
                "description": "test tool",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "input": {
                            "type": "string"
                        }
                    },
                    "required": ["input"]
                },
            }
        ]
    });

    let json_str = serde_json::to_string(&mcp_rust_sdk::Response::success(
        RequestId::Number(1),
        Some(tools.clone()),
    ))
    .expect("serialize response");

    let transport = Arc::new(Binary::mock(json_str, None));
    let client = Client::new_with_transport(transport);

    // Try to send notification with 5 second timeout
    let result = tokio::time::timeout(Duration::from_secs(5), client.list_tools()).await;

    // Should complete before timeout
    assert!(result.is_ok(), "Operation should complete before timeout");
    let resp = serde_json::to_value(&result.unwrap().unwrap()).unwrap();
    assert_eq!(resp, tools.get("tools").unwrap().clone());
}

#[tokio::test]
async fn test_client_call_tool_success() {
    let tools = json!({
        "content": [{
            "type": "text",
            "text": "127.0.0.1"
        }],
        "isError": false,
    });

    let json_str = serde_json::to_string(&mcp_rust_sdk::Response::success(
        RequestId::Number(1),
        Some(tools.clone()),
    ))
    .expect("serialize response");

    let transport = Arc::new(Binary::mock(json_str, None));
    let client = Client::new_with_transport(transport);

    // Try to send notification with 5 second timeout
    let result = tokio::time::timeout(Duration::from_secs(5), client.call_tool("myip", None)).await;

    // Should complete before timeout
    assert!(result.is_ok(), "Operation should complete before timeout");
    let text = match result.unwrap().unwrap().content[0] {
        CallToolResultContent::Text { ref text } => text.to_string(),
        _ => panic!("Expected text content"),
    };
    assert_eq!(text, "127.0.0.1");
}
