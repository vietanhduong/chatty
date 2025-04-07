use crate::{info_notice, task_failure, task_success};

use super::*;

#[tokio::test]
async fn test_initializer() {
    let handler = tokio::spawn(async move {
        let mut init = Initializer::default();
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
