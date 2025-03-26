use super::*;
use crate::backend::MockBackend;
use crate::models::{BackendResponse, BackendUsage, Conversation, Message, message::Issuer};

#[test]
fn test_find_checkpoint() {
    let mut convo = build_convo();
    let checkpoint = find_checkpoint(&convo, 3);
    assert_eq!(checkpoint, Some(12));
    convo.append_message(Message::new_user("user", "How are you doing?"));
    let checkpoint = find_checkpoint(&convo, 3);
    assert_eq!(checkpoint, Some(14));
}

#[tokio::test]
async fn test_compress() {
    let convo = build_convo();

    let mut messages = convo
        .contexts()
        .iter()
        .map(Message::from)
        .collect::<Vec<_>>();

    messages.extend(convo.messages()[6..11].to_vec());
    let expected_message = messages
        .iter()
        .map(|msg| format!("{}: {}", message_categorize(msg), msg.text()))
        .collect::<Vec<_>>()
        .join("\n");

    let expected_message = format!(
        r#"Summarize the following conversation in a compact yet comprehensive manner.
Focus on the key points, decisions, and any critical information exchanged, while omitting trivial or redundant details. Include specific actions or plans that were agreed upon.
Ensure that the summary is understandable on its own, providing enough context for someone who hasn't read the entire conversation.
Aim to capture the essence of the discussion while keeping the summary as concise as possible.
The summary should be started with Summary: and end with a period.
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
    assert_eq!(context.last_message_id(), "9");
}

fn build_convo() -> Conversation {
    let mut convo = Conversation::new_hello();
    for i in 0..=15 {
        let issuer = if i % 2 == 0 {
            Issuer::user_with_name("user")
        } else {
            Issuer::system_with_name("system")
        };
        convo.append_message(
            Message::new(issuer, format!("Message {}", i))
                .with_id(i.to_string())
                .with_token_count(5),
        );
    }
    convo.append_context(
        ConvoContext::new("4")
            .with_content("This is a checkpoint at 4")
            .with_token_count(5),
    );
    convo
}
