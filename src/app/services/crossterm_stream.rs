use crossterm::event::{Event, EventStream};
use std::pin::Pin;

type StreamOutput = Option<Result<Event, std::io::Error>>;

pub trait CrosstermStream: Send + Sync + 'static {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = StreamOutput> + Send + '_>>;
}

impl CrosstermStream for EventStream {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = StreamOutput> + Send + '_>> {
        Box::pin(futures::StreamExt::next(self))
    }
}
