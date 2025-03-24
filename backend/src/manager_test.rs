use super::*;
use crate::MockBackend;
use std::sync::Arc;

#[tokio::test]
async fn test_add_connection() {
    let mut mock = MockBackend::new();
    mock.expect_name().times(1).return_const("test".to_string());
    mock.expect_health_check()
        .times(1)
        .returning(|| Box::pin(async { Ok(()) }));
    mock.expect_list_models()
        .times(1)
        .returning(|_| Box::pin(async { Ok(vec!["model1".to_string(), "model2".to_string()]) }));
    let mut manager = Manager::default();
    let result = manager.add_connection(Arc::new(mock)).await;
    assert!(result.is_ok());
    assert_eq!(manager.models.len(), 2);
    assert_eq!(manager.models.get("model1"), Some(&"test".to_string()));
    assert_eq!(manager.models.get("model2"), Some(&"test".to_string()));

    assert_eq!(manager.connections.len(), 1);
    assert!(manager.connections.contains_key("test"));
}

#[tokio::test]
async fn test_add_connection_with_error() {
    let mut mock = MockBackend::new();
    mock.expect_name().times(1).return_const("test".to_string());
    mock.expect_health_check()
        .times(1)
        .returning(|| Box::pin(async { Err(eyre::eyre!("test error")) }));
    let mut manager = Manager::default();
    let result = manager.add_connection(Arc::new(mock)).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.to_string(), "health check backend test");
    let root_cause = err.root_cause();
    assert_eq!(root_cause.to_string(), "test error");
}
