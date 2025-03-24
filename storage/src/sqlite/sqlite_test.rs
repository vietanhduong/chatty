use super::*;

#[test]
fn test_filter_to_query() {
    let mut filter = FilterConversation::default().with_id("test_id");

    let (query, params) = filter_to_query(&filter);
    assert_eq!(
        query,
        "SELECT id, title, created_at, updated_at FROM conversations WHERE 1=1 AND id = :id"
    );

    assert_eq!(params.len(), 1);
    assert_eq!(params[0].0, ":id");

    filter = filter.with_title("test");
    let (query, params) = filter_to_query(&filter);
    assert_eq!(
        query,
        "SELECT id, title, created_at, updated_at FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title"
    );
    assert_eq!(params.len(), 2);
    assert_eq!(params[0].0, ":id");
    assert_eq!(params[1].0, ":title");

    filter = filter.with_message_contains("test");
    let (query, params) = filter_to_query(&filter);
    assert_eq!(
        query,
        "SELECT id, title, created_at, updated_at FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title AND EXISTS (SELECT 1 FROM messages WHERE conversation_id = conversations.id AND text LIKE :message_contains)"
    );

    assert_eq!(params.len(), 3);
    assert_eq!(params[0].0, ":id");
    assert_eq!(params[1].0, ":title");
    assert_eq!(params[2].0, ":message_contains");

    filter = filter.with_created_at_from(chrono::Utc::now());
    let (query, params) = filter_to_query(&filter);
    assert_eq!(
        query,
        "SELECT id, title, created_at, updated_at FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title AND EXISTS (SELECT 1 FROM messages WHERE conversation_id = conversations.id AND text LIKE :message_contains) AND created_at >= :created_at_from"
    );
    assert_eq!(params.len(), 4);
    assert_eq!(params[0].0, ":id");
    assert_eq!(params[1].0, ":title");
    assert_eq!(params[2].0, ":message_contains");
    assert_eq!(params[3].0, ":created_at_from");

    filter = filter.with_updated_at_to(chrono::Utc::now());
    let (query, params) = filter_to_query(&filter);
    assert_eq!(
        query,
        "SELECT id, title, created_at, updated_at FROM conversations WHERE 1=1 AND id = :id AND title LIKE :title AND EXISTS (SELECT 1 FROM messages WHERE conversation_id = conversations.id AND text LIKE :message_contains) AND updated_at <= :updated_at_to AND created_at >= :created_at_from"
    );
    assert_eq!(params.len(), 5);
    assert_eq!(params[0].0, ":id");
    assert_eq!(params[1].0, ":title");
    assert_eq!(params[2].0, ":message_contains");
    assert_eq!(params[3].0, ":updated_at_to");
    assert_eq!(params[4].0, ":created_at_from");
}

#[tokio::test]
async fn test_upsert_conversation() {
    let db = Sqlite::new(None).await.unwrap();
    db.run_migration().await.unwrap();

    let expected = Conversation::default()
        .with_id("test_id")
        .with_title("Test Conversation")
        .with_created_at(chrono::Utc::now());

    db.upsert_conversation(expected.clone()).await.unwrap();

    let actual = db.get_conversation("test_id").await.unwrap();
    assert!(actual.is_some());

    let actual = actual.unwrap();
    assert_eq!(actual.id(), "test_id");
    assert_eq!(actual.title(), "Test Conversation");
    assert_eq!(
        actual.created_at().timestamp_millis(),
        expected.created_at().timestamp_millis()
    );
    assert_eq!(actual.messages().len(), 0);
}

#[tokio::test]
async fn test_insert_conversation_with_messages() {
    let db = Sqlite::new(None).await.unwrap();
    db.run_migration().await.unwrap();

    let mut expected = Conversation::default()
        .with_id("test_id")
        .with_title("Test Conversation")
        .with_created_at(chrono::Utc::now());

    db.upsert_conversation(expected.clone()).await.unwrap();

    let actual = db.get_conversation("test_id").await.unwrap();
    assert!(actual.is_some());

    let actual = actual.unwrap();
    assert_eq!(actual.id(), "test_id");
    assert_eq!(actual.title(), "Test Conversation");
    assert_eq!(
        actual.created_at().timestamp_millis(),
        expected.created_at().timestamp_millis()
    );
    assert_eq!(
        actual.updated_at().timestamp_millis(),
        expected.updated_at().timestamp_millis()
    );

    assert_eq!(actual.messages().len(), 0);

    expected.set_title("Updated Title");

    db.upsert_conversation(expected.clone()).await.unwrap();

    let actual = db.get_conversation("test_id").await.unwrap();
    assert!(actual.is_some());
    let actual = actual.unwrap();
    assert_eq!(actual.id(), "test_id");
    assert_eq!(actual.title(), "Updated Title");
}

#[tokio::test]
async fn test_add_messages() {
    let db = Sqlite::new(None).await.unwrap();
    db.run_migration().await.unwrap();

    let messages = vec![
        Message::new_system("system", "System message")
            .with_id("msg1")
            .with_created_at(chrono::Utc::now())
            .with_token_count(5),
        Message::new_user("user", "User message")
            .with_id("msg2")
            .with_created_at(chrono::Utc::now())
            .with_token_count(2),
    ];

    let conversation = Conversation::default()
        .with_id("test_id")
        .with_title("Test Conversation")
        .with_created_at(chrono::Utc::now())
        .with_messages(messages.clone());

    db.upsert_conversation(conversation.clone()).await.unwrap();
    db.add_messages(conversation.id(), conversation.messages())
        .await
        .unwrap();

    let message = Message::new_user("user", "hello")
        .with_id("msg3")
        .with_created_at(chrono::Utc::now())
        .with_token_count(10);

    db.add_messages(conversation.id(), &vec![message.clone()])
        .await
        .unwrap();

    let actual = db.get_conversation(conversation.id()).await.unwrap();
    assert!(actual.is_some());

    let actual = actual.unwrap();
    assert_eq!(actual.id(), conversation.id());
    assert_eq!(actual.messages().len(), 3);
    assert_eq!(actual.messages()[2].id(), "msg3");
    assert_eq!(actual.messages()[2].text(), "hello");
    assert_eq!(actual.messages()[2].issuer_str(), "user");
    assert_eq!(actual.messages()[2].is_system(), false);
    assert_eq!(actual.messages()[2].token_count(), 10);
    assert_eq!(
        actual.messages()[2].created_at().timestamp_millis(),
        message.created_at().timestamp_millis()
    );
}

#[tokio::test]
async fn test_delete_conversation() {
    let db = Sqlite::new(None).await.unwrap();
    db.run_migration().await.unwrap();

    let messages = vec![
        Message::new_system("system", "System message")
            .with_id("msg1")
            .with_created_at(chrono::Utc::now()),
        Message::new_user("user", "User message")
            .with_id("msg2")
            .with_created_at(chrono::Utc::now()),
    ];

    let expected = Conversation::default()
        .with_id("test_id")
        .with_title("Test Conversation")
        .with_created_at(chrono::Utc::now())
        .with_messages(messages.clone());

    db.upsert_conversation(expected.clone()).await.unwrap();

    let actual = db.get_conversation("test_id").await.unwrap();
    assert!(actual.is_some());

    let actual = actual.unwrap();
    assert_eq!(actual.id(), "test_id");

    db.delete_conversation("test_id").await.unwrap();
    let actual = db.get_conversation("test_id").await.unwrap();
    assert!(actual.is_none());

    let actual = db.get_messages("test_id").await.unwrap();
    assert!(actual.is_empty());
}

#[tokio::test]
async fn test_get_conversation_not_exist() {
    let db = Sqlite::new(None).await.unwrap();
    db.run_migration().await.unwrap();

    let actual = db.get_conversation("non_existent_id").await.unwrap();
    assert!(actual.is_none());
}

#[tokio::test]
async fn test_get_conversations_with_filter() {
    let db = Sqlite::new(None).await.unwrap();
    db.run_migration().await.unwrap();

    let conversations = fake_converstations();
    for conversation in &conversations {
        db.upsert_conversation(conversation.clone()).await.unwrap();

        db.add_messages(conversation.id(), conversation.messages())
            .await
            .unwrap();
    }

    let filter = FilterConversation::default().with_id("test_id_0");
    let actual = db.get_conversations(filter).await.unwrap();

    let con = actual.get("test_id_0").unwrap();

    assert_eq!(con.id(), "test_id_0");
    assert_eq!(con.title(), "Even Conversation 0");

    let filter = FilterConversation::default().with_title("Odd");
    let actual = db.get_conversations(filter).await.unwrap();
    assert_eq!(actual.len(), 5);

    let expected_ids = vec![
        "test_id_1",
        "test_id_3",
        "test_id_5",
        "test_id_7",
        "test_id_9",
    ];

    for (_, id) in expected_ids.iter().enumerate() {
        let con = actual.get(*id).unwrap();
        assert_eq!(con.id(), *id);
    }

    let filter = FilterConversation::default().with_message_contains("System");
    let actual = db.get_conversations(filter).await.unwrap();

    let expected_ids = vec![
        "test_id_0",
        "test_id_2",
        "test_id_4",
        "test_id_6",
        "test_id_8",
    ];

    assert_eq!(actual.len(), 5);
    for (_, id) in expected_ids.iter().enumerate() {
        let con = actual.get(*id).unwrap();
        assert_eq!(con.id(), *id);
    }
}

fn fake_converstations() -> Vec<Conversation> {
    let mut conversations = vec![];
    for i in 0..10 {
        let message = if i % 2 == 0 {
            Message::new_system("system", "System message")
                .with_id(format!("msg1_{}", i))
                .with_created_at(chrono::Utc::now())
        } else {
            Message::new_user("user", "User message")
                .with_id(format!("msg2_{}", i))
                .with_created_at(chrono::Utc::now())
        };

        let messages = vec![
            message.clone(),
            message.clone().with_id(format!("msg3_{}", i)),
            message.clone().with_id(format!("msg4_{}", i)),
        ];

        let title = if i % 2 == 0 {
            "Even Conversation"
        } else {
            "Odd Conversation"
        };

        let conversation = Conversation::default()
            .with_id(format!("test_id_{}", i))
            .with_title(format!("{} {}", title, i))
            .with_created_at(chrono::Utc::now())
            .with_messages(messages);
        conversations.push(conversation);
    }
    conversations
}

#[tokio::test]
async fn test_upsert_message() {
    let db = Sqlite::new(None).await.unwrap();
    db.run_migration().await.unwrap();

    let mut message = Message::new_system("system", "System message")
        .with_id("msg1")
        .with_created_at(chrono::Utc::now());

    let conversation = Conversation::default()
        .with_id("test_id")
        .with_title("Test Conversation")
        .with_created_at(chrono::Utc::now())
        .with_messages(vec![message.clone()]);

    db.upsert_conversation(conversation.clone()).await.unwrap();

    db.add_messages(conversation.id(), &[message.clone()])
        .await
        .unwrap();

    message.append(" hello");
    db.upsert_message("test_id", message.clone()).await.unwrap();
    let actual = db.get_messages("test_id").await.unwrap();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].id(), "msg1");
    assert_eq!(actual[0].text(), "System message hello");

    db.upsert_message("test_id", message.clone().with_id("msg2"))
        .await
        .unwrap();
    let actual = db.get_messages("test_id").await.unwrap();
    assert_eq!(actual.len(), 2);
    assert_eq!(actual[1].id(), "msg2");
}

#[tokio::test]
async fn test_delete_message() {
    let db = Sqlite::new(None).await.unwrap();
    db.run_migration().await.unwrap();
    let mut message = Message::new_system("system", "System message")
        .with_id("msg1")
        .with_created_at(chrono::Utc::now());

    let conversation = Conversation::default()
        .with_id("test_id")
        .with_title("Test Conversation")
        .with_created_at(chrono::Utc::now())
        .with_messages(vec![message.clone()]);

    db.upsert_conversation(conversation.clone()).await.unwrap();

    db.add_messages(conversation.id(), &[message.clone()])
        .await
        .unwrap();

    message.append(" hello");
    db.upsert_message("test_id", message.clone()).await.unwrap();

    let actual = db.get_messages("test_id").await.unwrap();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].id(), "msg1");
    assert_eq!(actual[0].text(), "System message hello");

    db.delete_messsage("msg1").await.unwrap();
    let actual = db.get_messages("test_id").await.unwrap();
    assert_eq!(actual.len(), 0);
}
