#[cfg(test)]
#[path = "binary_transport_test.rs"]
mod tests;

use std::{collections::HashMap, pin::Pin, process::Stdio, sync::Arc, time::Duration};

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

use crate::config::{BinaryConfig, constants::BINARY_TRANSPORT_TIMEOUT_SECS};

pub struct BinaryTransportBuilder {
    filename: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    timeout: Option<Duration>,
}

impl BinaryTransportBuilder {
    pub fn new(filename: impl Into<String>) -> Self {
        Self {
            filename: filename.into(),
            args: Vec::new(),
            env: HashMap::new(),
            timeout: None,
        }
    }

    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn env(mut self, key: String, value: String) -> Self {
        self.env.insert(key, value);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn build(self) -> Result<BinaryTransport, Error> {
        let mut process = Command::new(self.filename)
            .args(self.args)
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

        Ok(BinaryTransport {
            stdin,
            stdout,
            process: Some(Arc::new(Mutex::new(process))),
            timeout: self.timeout,
        })
    }
}

pub struct BinaryTransport {
    stdin: Arc<Mutex<Box<dyn AsyncWrite + Send + Unpin>>>,
    stdout: Arc<Mutex<BufReader<Box<dyn AsyncRead + Send + Unpin>>>>,
    process: Option<Arc<Mutex<tokio::process::Child>>>, // Optional to allow for mocking
    timeout: Option<Duration>,
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
        let timeout = self
            .timeout
            .clone()
            .unwrap_or(Duration::from_secs(BINARY_TRANSPORT_TIMEOUT_SECS));
        Box::pin(futures::stream::unfold(stdout, move |stdout| {
            let timeout = timeout;
            async move {
                let mut line = String::new();
                let read_line = {
                    let mut reader = stdout.lock().await;
                    tokio::time::timeout(timeout, reader.read_line(&mut line)).await
                };

                if let Err(e) = read_line {
                    return Some((Err(Error::Io(e.to_string())), stdout));
                }

                match read_line.unwrap() {
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
            }
        }))
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

impl From<&BinaryConfig> for BinaryTransportBuilder {
    fn from(config: &BinaryConfig) -> Self {
        let mut builder = BinaryTransportBuilder::new(&config.filename);
        builder = builder.args(config.args.clone());

        for (key, value) in &config.env {
            builder = builder.env(key.clone(), value.clone());
        }
        if let Some(timeout) = config.timeout_secs {
            builder = builder.timeout(Duration::from_secs(timeout as u64));
        }
        builder
    }
}
