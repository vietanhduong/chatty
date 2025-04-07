use crossterm::event::{Event, EventStream};
use std::pin::Pin;

pub trait CrosstermStream: Send + Sync + 'static {
    fn next(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Option<Result<Event, std::io::Error>>> + Send + '_>>;
}

impl CrosstermStream for EventStream {
    fn next(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Option<Result<Event, std::io::Error>>> + Send + '_>> {
        Box::pin(futures::StreamExt::next(self))
    }
}
