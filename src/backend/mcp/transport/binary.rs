#[cfg(test)]
#[path = "binary_test.rs"]
mod tests;

use std::{pin::Pin, process::Stdio, sync::Arc};

use futures::Stream;
use mcp_rust_sdk::{
    Error, Response,
    transport::{Message, Transport},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    process::Command,
    sync::Mutex,
};

use crate::config::BinaryConfig;

pub struct Binary {
    stdin: Arc<Mutex<Box<dyn AsyncWrite + Send + Unpin>>>,
    stdout: Arc<Mutex<BufReader<Box<dyn AsyncRead + Send + Unpin>>>>,
    process: Option<Arc<Mutex<tokio::process::Child>>>, // Optional to allow for mocking
}

impl Binary {
    pub fn new(config: &BinaryConfig) -> Result<Self, Error> {
        let mut process = Command::new(&config.filename)
            .args(&config.args)
            .envs(&config.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = Arc::new(Mutex::new(Box::new(
            process
                .stdin
                .take()
                .ok_or_else(|| Error::Io("failed to open stdin".to_string()))?,
        ) as Box<dyn AsyncWrite + Send + Unpin>));

        let stdout = Arc::new(Mutex::new(BufReader::new(Box::new(
            process
                .stdout
                .take()
                .ok_or_else(|| Error::Io("failed to open stdout".to_string()))?,
        )
            as Box<dyn AsyncRead + Send + Unpin>)));

        Ok(Binary {
            stdin,
            stdout,
            process: Some(Arc::new(Mutex::new(process))),
        })
    }
}

#[async_trait::async_trait]
impl Transport for Binary {
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

        let stream = futures::stream::unfold(
            (stdout, String::new()),
            move |(stdout, mut buffer)| async move {
                buffer.clear();

                let read_result = {
                    let mut stdout_guard = stdout.lock().await;
                    stdout_guard.read_line(&mut buffer).await
                };

                match read_result {
                    Ok(0) => None, // EOF
                    Ok(_) => {
                        let resp: Response = match serde_json::from_str(&buffer) {
                            Ok(resp) => resp,
                            Err(e) => {
                                return Some((
                                    Err(Error::Serialization(e.to_string())),
                                    (stdout, buffer),
                                ));
                            }
                        };
                        Some((Ok(Message::Response(resp)), (stdout, buffer)))
                    }
                    Err(e) => Some((Err(Error::Io(e.to_string())), (stdout, buffer))),
                }
            },
        );
        Box::pin(stream)
    }

    /// Close the transport
    async fn close(&self) -> Result<(), Error> {
        if let Some(process) = &self.process {
            let mut process = process.lock().await;
            process.kill().await?;
        }
        Ok(())
    }
}
