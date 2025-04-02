use std::{task::Poll, time::Duration};

use futures::StreamExt;
use mcp_rust_sdk::{Request, protocol::RequestId};
use serde_json::json;
use tokio::time::{sleep, timeout};

use super::*;

impl Binary {
    fn mock(read_data: impl Into<String>, delay: Option<Duration>) -> Self {
        let mock_stream = MockStream::new(read_data.into().as_bytes())
            .with_delay(delay.unwrap_or(Duration::from_millis(0)));
        let stdout = Arc::new(Mutex::new(BufReader::new(
            Box::new(mock_stream) as Box<dyn AsyncRead + Send + Unpin>
        )));
        let stdin = Arc::new(Mutex::new(
            Box::new(MockStream::new(&vec![])) as Box<dyn AsyncWrite + Send + Unpin>
        ));
        Self {
            stdin,
            stdout,
            process: None,
        }
    }
}

#[tokio::test]
async fn test_normal_case() {
    let tools = json!({
        "tools": [
            {
                "name": "test_tool",
                "description": "test tool"
            }
        ]
    });

    let json_str = serde_json::to_string(&mcp_rust_sdk::Response::success(
        RequestId::String("init_request_1".to_string()),
        Some(tools.clone()),
    ))
    .expect("serialize response");

    let transport = Binary::mock(json_str, None);

    transport
        .send(Message::Request(Request::new(
            "tools/list",
            None,
            RequestId::String("init_request_1".to_string()),
        )))
        .await
        .expect("send request");

    let mut result = transport.receive();

    let resp = timeout(Duration::from_secs(5), result.next()).await;
    assert!(resp.is_ok());
    let resp = resp.unwrap().unwrap().expect("response");
    let json_value = match resp {
        Message::Response(resp) => resp.result,
        _ => panic!("expected response"),
    };
    assert_eq!(json_value, Some(tools));
}

#[tokio::test]
async fn test_request_timeout() {
    let tools = json!({
        "tools": [
            {
                "name": "test_tool",
                "description": "test tool"
            }
        ]
    });

    let json_str = serde_json::to_string(&mcp_rust_sdk::Response::success(
        RequestId::String("init_request_1".to_string()),
        Some(tools.clone()),
    ))
    .expect("serialize response");

    let transport = Binary::mock(json_str, Some(Duration::from_secs(2)));

    transport
        .send(Message::Request(Request::new(
            "tools/list",
            None,
            RequestId::String("init_request_1".to_string()),
        )))
        .await
        .expect("send request");

    let mut result = transport.receive();

    let resp = timeout(Duration::from_secs(5), result.next()).await;
    assert!(resp.is_ok());
    let resp = resp.unwrap().unwrap().err().expect("response error");
    assert!(resp.to_string().contains("deadline has elapsed"));
}

struct MockStream {
    read_data: Vec<u8>,
    write_data: Vec<u8>,
    pos: usize,
    delay: Duration,
}

impl MockStream {
    fn new(read_data: &[u8]) -> Self {
        Self {
            read_data: read_data.to_vec(),
            write_data: Vec::new(),
            pos: 0,
            delay: Duration::from_millis(0),
        }
    }

    #[allow(dead_code)]
    pub fn written_data(&self) -> &[u8] {
        &self.write_data
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

impl AsyncRead for MockStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.pos >= self.read_data.len() {
            return Poll::Ready(Ok(()));
        }

        let mut future = Box::pin(sleep(self.delay));
        let n = std::cmp::min(buf.remaining(), self.read_data.len() - self.pos);
        match future.as_mut().poll(cx) {
            Poll::Ready(_) => {
                buf.put_slice(&self.read_data[self.pos..self.pos + n]);
                self.pos += n;
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for MockStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.write_data.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }
}
