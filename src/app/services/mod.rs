pub mod action;
pub mod clipboard;
pub mod crossterm_stream;
pub mod events;

pub use clipboard::ClipboardService;
pub use crossterm_stream::CrosstermStream;
pub use events::EventService;

use std::sync::{Arc, atomic};

use eyre::{Result, eyre};
use std::time::Duration;
use tokio::sync::oneshot;

pub struct ShutdownCoordinator {
    pub pending_tasks: Arc<atomic::AtomicUsize>,
    pub shutdown_complete: oneshot::Sender<Result<()>>,
    pub timeout: Option<Duration>,
}

impl ShutdownCoordinator {
    pub async fn wait_for_completion(self) -> Result<()> {
        let timeout = self.timeout.unwrap_or(Duration::from_secs(15));
        let result = match tokio::time::timeout(timeout, self.wait_pending_tasks()).await {
            Ok(_) => Ok(()),
            Err(_) => Err(eyre!("shutdown timeout reached")),
        };
        let _ = self.shutdown_complete.send(result);
        Ok(())
    }

    async fn wait_pending_tasks(&self) {
        while self.pending_tasks.load(atomic::Ordering::SeqCst) > 0 {
            log::debug!(
                "Waiting for {} pending tasks",
                self.pending_tasks.load(atomic::Ordering::SeqCst)
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
