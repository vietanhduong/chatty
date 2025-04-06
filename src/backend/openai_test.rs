use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::*;

#[tokio::test]
async fn test_list_models() {
    let body = serde_json::to_string(&ModelListResponse {
        data: vec![
            ModelResponse {
                id: "gpt-3.5-turbo".to_string(),
            },
            ModelResponse {
                id: "gpt-4".to_string(),
            },
            ModelResponse {
                id: "o1-mini".to_string(),
            },
        ],
    });

    let mut server = mockito::Server::new_async().await;

    let models_handler = server
        .mock("GET", "/v1/models")
        .with_status(200)
        .match_header("Authorization", "Bearer test_token")
        .with_body(body.unwrap())
        .expect_at_most(1)
        .create();

    let backend = OpenAI::default()
        .with_endpoint(&server.url())
        .with_api_key("test_token")
        .with_want_models(vec!["gpt-3.5-turbo".to_string(), "gpt-4".to_string()]);

    let res = backend.list_models().await.expect("Failed to list models");

    assert_eq!(res.len(), 2);
    assert_eq!(res[0].id(), "gpt-3.5-turbo");
    assert_eq!(res[1].id(), "gpt-4");
    models_handler.assert();
}

#[tokio::test]
async fn test_get_completion() {
    let mut lines = ["Hello ".to_string(), "there!".to_string()]
        .into_iter()
        .map(|l| CompletionResponse {
            id: uuid::Uuid::new_v4().to_string(),
            choices: vec![CompletionChoiceResponse {
                delta: CompletionDeltaResponse {
                    content: Some(l),
                    ..Default::default()
                },
                finish_reason: None,
            }],
            ..Default::default()
        })
        .collect::<Vec<_>>();

    lines.push(CompletionResponse {
        id: uuid::Uuid::new_v4().to_string(),
        choices: vec![CompletionChoiceResponse {
            delta: CompletionDeltaResponse {
                content: None,
                ..Default::default()
            },
            finish_reason: Some("stop".to_string()),
        }],
        ..Default::default()
    });

    let mut lines = lines
        .into_iter()
        .map(|l| {
            format!(
                "data: {}",
                serde_json::to_string(&l).expect("Failed to serialize")
            )
        })
        .collect::<Vec<_>>();
    lines.push("data: [DONE]".to_string());
    let body = lines.join("\n");

    let prompt = BackendPrompt::new("Hello").with_model("gpt-3.5-turbo");

    let mut server = mockito::Server::new_async().await;
    let completion_handler = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .match_header("Authorization", "Bearer test_token")
        .with_body(body)
        .create();

    let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
    let sender: ArcEventTx = Arc::new(tx);

    let backend = setup_backend(server.url()).await;

    backend
        .get_completion(prompt, sender)
        .await
        .expect("Failed to get completion");
    completion_handler.assert();

    let events = collect_responses(&mut rx, time::Duration::from_secs(5), 3)
        .await
        .expect("Failed to collect events");
    assert_eq!(events.len(), 3);

    assert_eq!(events[0].text, "Hello ");
    assert_eq!(events[0].done, false);
    assert_eq!(events[0].init_conversation, true);
    assert_eq!(events[1].text, "there!");
    assert_eq!(events[1].done, false);
    assert_eq!(events[1].init_conversation, true);
    assert_eq!(events[2].text, "");
    assert_eq!(events[2].done, true);
    assert_eq!(events[2].init_conversation, true);
}

async fn collect_responses(
    rx: &mut UnboundedReceiver<Event>,
    timeout: time::Duration,
    want_len: usize,
) -> Result<Vec<BackendResponse>> {
    let mut responses = Vec::new();
    let start = time::Instant::now();
    while responses.len() < want_len {
        if let Some(event) = rx.recv().await {
            match event {
                Event::ChatCompletionResponse(msg) => responses.push(msg),
                event => bail!("Unexpected event: {:?}", event),
            }
        }
        if start.elapsed() > timeout {
            return Err(eyre::eyre!("Timeout while waiting for events"));
        }
    }
    Ok(responses)
}

async fn setup_backend(url: String) -> OpenAI {
    let backend = OpenAI::default()
        .with_endpoint(&url)
        .with_api_key("test_token")
        .with_want_models(vec!["gpt-3.5-turbo".to_string(), "gpt-4".to_string()]);
    backend
}
