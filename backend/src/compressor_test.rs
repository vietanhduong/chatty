use super::*;
use crate::MockBackend;
use openai_models::{BackendResponse, BackendUsage, Conversation, Message};

#[test]
fn test_find_checkpoint() {
    let mut convo = build_convo();
    let checkpoint = find_checkpoint(&convo, 3);
    assert_eq!(checkpoint, Some(5));
    convo.add_message(Message::new_user("user", "How are you doing?"));
    let checkpoint = find_checkpoint(&convo, 3);
    assert_eq!(checkpoint, Some(7));
}

#[tokio::test]
async fn test_compress() {
    let convo = build_convo();

    let expected_message = convo.messages()[..4]
        .iter()
        .map(|msg| {
            format!(
                "{}: {}",
                if msg.is_system() { "System" } else { "User" },
                msg.text()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let expected_message = format!(
        r#"Summarize the following conversation in a compact yet comprehensive manner. Focus on the key points, decisions, and any critical information exchanged, while omitting trivial or redundant details. Include specific actions or plans that were agreed upon. Ensure that the summary is understandable on its own, providing enough context for someone who hasn't read the entire conversation. Aim to capture the essence of the discussion while keeping the summary as concise as possible.
---
{}"#,
        expected_message
    );

    let mut backend = MockBackend::new();
    backend
        .expect_get_completion()
        .returning(move |prompt, event_tx| {
            let expected_message = expected_message.clone();
            Box::pin(async move {
                assert_eq!(prompt.text(), expected_message);
                let resp = ["This ", "is ", "a ", "compressed ", "context", ""]
                    .iter()
                    .map(|s| BackendResponse {
                        done: *s == "",
                        id: "test_id".to_string(),
                        text: s.to_string(),
                        model: "test_model".to_string(),
                        init_conversation: false,
                        usage: if *s == "" {
                            Some(BackendUsage {
                                completion_tokens: 5,
                                prompt_tokens: 10,
                                total_tokens: 15,
                            })
                        } else {
                            None
                        },
                    })
                    .collect::<Vec<_>>();
                for msg in resp {
                    event_tx
                        .send(Event::BackendPromptResponse(msg))
                        .await
                        .expect("Failed to send event");
                }
                Ok(())
            })
        });

    let compressor = Compressor::new(Arc::new(backend))
        .with_context_length(10)
        .with_conversation_length(10);

    assert!(compressor.should_compress(&convo));

    let context = compressor
        .compress("test_model", &convo)
        .await
        .expect("Failed to compress conversation")
        .unwrap();

    assert_eq!(context.id(), "test_id");
    assert_eq!(context.content(), "This is a compressed context");
    assert_eq!(context.token_count(), 5);
    assert_eq!(context.last_message_id(), "4");
}

fn build_convo() -> Conversation {
    Conversation::new_hello().with_messages(vec![
        Message::new_user("user", "Hello")
            .with_token_count(1)
            .with_id("1"),
        Message::new_system("assistant", "Hi! How can I help you?")
            .with_token_count(5)
            .with_id("2"),
        Message::new_user("user", "Can you tell me a joke?")
            .with_token_count(5)
            .with_id("3"),
        Message::new_system("assistant", "Sure! Why did the chicken cross the road?")
            .with_token_count(7)
            .with_id("4"),
        Message::new_user("user", "To get to the other side!")
            .with_token_count(5)
            .with_id("5"),
        Message::new_system("assistant", "Haha! That's a classic!")
            .with_token_count(5)
            .with_id("6"),
        Message::new_user("user", "What's your favorite color?")
            .with_token_count(5)
            .with_id("7"),
        Message::new_system("assistant", "I like blue!")
            .with_token_count(3)
            .with_id("8"),
        Message::new_user("user", "Do you have any hobbies?")
            .with_token_count(5)
            .with_id("9"),
        Message::new_system("assistant", "I enjoy learning new things!")
            .with_token_count(5)
            .with_id("10"),
    ])
}
