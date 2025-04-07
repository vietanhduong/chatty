use tokio::sync::Mutex;

use crate::{info_notice, task_failure, task_success};

use super::*;

impl Initializer {
    fn mock(events: Vec<CrosstermEvent>) -> Self {
        let (task_tx, task_rx) = mpsc::unbounded_channel::<TaskEvent>();
        SENDER.set(task_tx).unwrap();

        Self {
            crossterm_events: Box::new(EventMock::new(events)),
            task_rx,
            messages: vec![],
            complete: false,
        }
    }
}

#[tokio::test]
async fn test_initializer() {
    let handler = tokio::spawn(async move {
        let mut init = Initializer::mock(vec![]);

        loop {
            let event = init.next_event().await;
            if init.handle_event(event) {
                return init;
            }
        }
    });
    while !Initializer::ready() {
        println!("Waiting for initializer to be ready...");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    Initializer::add_task("test_task1", "Test task");
    Initializer::add_task("test_task2", "Test task 2");
    Initializer::add_task("test_task3", "Test task 3");
    task_success!("test_task1");
    task_success!("test_task2", "Test task 2 success".to_string());
    task_failure!("test_task3", "Test task 3 failed".to_string());

    Initializer::add_notice(info_notice!("Test info notice"));
    Initializer::complete();
    let result = handler.await.unwrap();
    assert_eq!(result.messages.len(), 4, "Expected 4 messages");
    let completed_tasks = result
        .messages
        .iter()
        .filter(|m| matches!(m, Event::Task { completed_at, .. } if Some(completed_at).is_some()))
        .count();
    assert_eq!(completed_tasks, 3, "Expected 3 completed tasks");
}

struct EventMock {
    events: Mutex<Vec<CrosstermEvent>>,
}

impl EventMock {
    fn new(events: Vec<CrosstermEvent>) -> Self {
        Self {
            events: Mutex::new(events),
        }
    }
}

impl CrosstermStream for EventMock {
    fn next(
        &mut self,
    ) -> std::pin::Pin<
        Box<
            dyn Future<Output = Option<std::result::Result<CrosstermEvent, std::io::Error>>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move { self.events.lock().await.pop().map(|event| Ok(event)) })
    }
}
