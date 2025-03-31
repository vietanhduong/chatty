use std::time::Duration;

use futures::StreamExt;
use mcp_rust_sdk::{Request, protocol::RequestId};
use tokio::time;

use super::*;

#[tokio::test]
async fn test_request() {
    let path = "~/projects/hyper-mcp/target/release/hyper-mcp";
    let transport =
        BinaryTransport::new(path, &["-l".to_string(), "/tmp/mcp.log".to_string()]).unwrap();

    println!("prcess id: {:?}", transport.process.lock().await.id());
    transport
        .send(Message::Request(Request::new(
            "tools/list",
            None,
            RequestId::String("init_request_1".to_string()),
        )))
        .await
        .expect("send request");
    println!("request sent");

    let mut result = transport.receive();

    tokio::select! {
        _ = time::sleep(Duration::from_secs(10)) => {
            println!("timeout");
            return;
        }

        resp = result.next() => {
            match resp {
                Some(Ok(msg)) => {
                    if let Message::Response(resp) = msg {
                        println!("response: {:?}", resp);
                    }
                }
                Some(Err(err)) => {
                    println!("error: {:?}", err);
                }
                None => {
                    println!("no response");
                }
            }
        }
    }
}
