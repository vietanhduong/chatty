use super::*;
use crate::backend::mcp::{MockMcpClient, ToolInputSchema};

#[tokio::test]
async fn test_add_server() {
    let mut mock_mcp = MockMcpClient::new();
    mock_mcp.expect_list_tools().returning(|| {
        Box::pin(async {
            Ok(vec![
                fake_tool("test_tool", "test tool"),
                fake_tool("another_tool", "another tool"),
                fake_tool("yet_another_tool", "yet another tool"),
            ])
        })
    });

    let tmp = Arc::new(MockMcpClient::new());

    let mut manager = Manager::default();
    manager
        .tools
        .insert(fake_tool("another_tool", "replace"), tmp.clone());

    manager.tools.insert(
        fake_tool("yet_another_tool", "this tool will be not replace"),
        tmp.clone(),
    );
    let arc = Arc::new(mock_mcp);
    manager.add_server(arc.clone()).await.expect("add server");

    assert_eq!(
        manager.tools.len(),
        3,
        "Expected 3 tools after adding server"
    );

    let tools = manager.tools.keys().clone().collect::<Vec<_>>();
    let test_tool = tools.iter().find(|t| t.name == "test_tool").unwrap();
    assert_eq!(test_tool.name, "test_tool", "Expected test_tool");
    assert_eq!(
        test_tool.description.as_deref(),
        Some("test tool"),
        "Expected description to be 'test tool'"
    );

    let another_tool = tools.iter().find(|t| t.name == "another_tool").unwrap();
    assert_eq!(another_tool.name, "another_tool", "Expected another_tool");
    assert_eq!(
        another_tool.description.as_deref(),
        Some("another tool"),
        "Expected description to be 'another tool'"
    );

    let yet_another_tool = tools.iter().find(|t| t.name == "yet_another_tool").unwrap();
    assert_eq!(
        yet_another_tool.name, "yet_another_tool",
        "Expected yet_another_tool"
    );
    assert_eq!(
        yet_another_tool.description.as_deref(),
        Some("this tool will be not replace"),
        "Expected description to be 'yet another tool'"
    );
}

fn fake_tool(name: &str, desc: &str) -> Tool {
    Tool {
        name: name.to_string(),
        description: Some(desc.to_string()),
        input_schema: ToolInputSchema::default(),
    }
}
