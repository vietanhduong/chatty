use eyre::{Result, bail};
use once_cell::sync::OnceCell;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

static SENDER: OnceCell<mpsc::UnboundedSender<String>> = OnceCell::new();

pub struct ClipboardService;

impl ClipboardService {
    pub async fn start(cancel_token: CancellationToken) -> Result<()> {
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        SENDER.set(tx).unwrap();
        let mut clipboard = arboard::Clipboard::new()?;

        log::debug!("Clipboard service started");
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    log::debug!("Clipboard service cancelled");
                    return Ok(());
                }
                event = rx.recv() => {
                    if event.is_none() {
                        continue;
                    }
                    clipboard.set_text(event.unwrap())?;
                }
            }
        }
    }

    pub fn init() -> Result<()> {
        if SENDER.get().is_none() {
            arboard::Clipboard::new()?;
        }
        Ok(())
    }

    pub fn set(text: impl Into<String>) -> Result<()> {
        if let Some(tx) = SENDER.get() {
            tx.send(text.into())?;
            return Ok(());
        }

        bail!("clipboard service is not initialized")
    }
}
