#[cfg(test)]
#[path = "binary_transport_test.rs"]
mod tests;

use std::{pin::Pin, process::Stdio, sync::Arc};

use futures::Stream;
use mcp_rust_sdk::{
    Error, Response,
    transport::{Message, Transport},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::Mutex,
};

pub struct BinaryTransport {
    stdin: Arc<Mutex<tokio::process::ChildStdin>>,
    stdout: Arc<Mutex<BufReader<tokio::process::ChildStdout>>>,
    process: Arc<Mutex<tokio::process::Child>>,
}

impl BinaryTransport {
    pub fn new(path: impl Into<String>, args: &[String]) -> Result<Self, Error> {
        let mut process = Command::new(path.into())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = Arc::new(Mutex::new(
            process
                .stdin
                .take()
                .ok_or_else(|| Error::Io("failed to open stdin".to_string()))?,
        ));

        let stdout = Arc::new(Mutex::new(BufReader::new(
            process
                .stdout
                .take()
                .ok_or_else(|| Error::Io("failed to open stdout".to_string()))?,
        )));

        Ok(Self {
            stdin,
            stdout,
            process: Arc::new(Mutex::new(process)),
        })
    }
}

#[async_trait::async_trait]
impl Transport for BinaryTransport {
    /// Send a message over the transport
    async fn send(&self, message: Message) -> Result<(), Error> {
        let mut stdin = self.stdin.lock().await;
        let json_str = serde_json::to_string(&message)? + "\n";
        stdin.write_all(json_str.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Receive messages from the transport
    fn receive(&self) -> Pin<Box<dyn Stream<Item = Result<Message, Error>> + Send>> {
        let stdout = Arc::clone(&self.stdout);
        Box::pin(futures::stream::unfold(stdout, |stdout| async move {
            let mut line = String::new();
            let read_line = {
                let mut reader = stdout.lock().await;
                reader.read_line(&mut line).await
            };

            match read_line {
                Ok(0) => None,
                Ok(_) => {
                    let message = match serde_json::from_str::<Response>(&line) {
                        Ok(resp) => Ok(Message::Response(resp)),
                        Err(e) => Err(Error::Serialization(format!(
                            "failed to parse response {}: {}",
                            line, e
                        ))),
                    };
                    Some((message, stdout))
                }
                Err(e) => Some((Err(Error::Io(e.to_string())), stdout)),
            }
        }))
    }

    /// Close the transport
    async fn close(&self) -> Result<(), Error> {
        let mut process = self.process.lock().await;
        Ok(process.start_kill()?)
    }
}
