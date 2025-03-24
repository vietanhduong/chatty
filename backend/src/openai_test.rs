use tokio::sync::mpsc::UnboundedReceiver;

use super::*;

impl OpenAI {
    async fn set_models(&self, models: Vec<String>) {
        let mut write = self.cache_models.write().await;
        *write = models;
    }
}

#[tokio::test]
async fn test_list_models() {
    let body = serde_json::to_string(&ModelListResponse {
        data: vec![
            Model {
                id: "gpt-3.5-turbo".to_string(),
            },
            Model {
                id: "gpt-4".to_string(),
            },
            Model {
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

    let res = backend
        .list_models(false)
        .await
        .expect("Failed to list models");

    assert_eq!(res.len(), 2);
    assert_eq!(res[0], "gpt-3.5-turbo");
    assert_eq!(res[1], "gpt-4");

    models_handler.assert();

    let res = backend
        .list_models(false)
        .await
        .expect("Failed to list models");

    assert_eq!(res.len(), 2);
    assert_eq!(res[0], "gpt-3.5-turbo");
    assert_eq!(res[1], "gpt-4");
    // Hit cache, no request to server
    models_handler.assert();
}

#[tokio::test]
async fn test_set_current_model() {
    let backend = OpenAI::default();

    backend
        .set_models(vec!["gpt-3.5-turbo".to_string(), "gpt-4".to_string()])
        .await;

    backend
        .set_current_model("gpt-3.5-turbo")
        .await
        .expect("Failed to set current model");

    assert_eq!(
        backend.current_model().await,
        Some("gpt-3.5-turbo".to_string())
    );

    let err = backend.set_current_model("o1-mini").await.unwrap_err();
    assert!(err.to_string().contains("model o1-mini not available"));
}

#[tokio::test]
async fn test_get_completion() {
    let mut lines = ["Hello ".to_string(), "there!".to_string()]
        .into_iter()
        .map(|l| CompletionResponse {
            id: uuid::Uuid::new_v4().to_string(),
            choices: vec![CompletionChoiceResponse {
                delta: CompletionDeltaResponse { content: Some(l) },
                finish_reason: None,
            }],
            ..Default::default()
        })
        .collect::<Vec<_>>();

    lines.push(CompletionResponse {
        id: uuid::Uuid::new_v4().to_string(),
        choices: vec![CompletionChoiceResponse {
            delta: CompletionDeltaResponse { content: None },
            finish_reason: Some("stop".to_string()),
        }],
        ..Default::default()
    });

    let body = lines
        .into_iter()
        .map(|l| serde_json::to_string(&l).expect("Failed to serialize"))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = BackendPrompt::new("Hello").with_model("gpt-3.5-turbo");

    let mut server = mockito::Server::new_async().await;
    let completion_handler = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .match_header("Authorization", "Bearer test_token")
        .with_body(body)
        .create();

    let (tx, mut rx) = mpsc::unbounded_channel::<Event>();

    let backend = setup_backend(server.url()).await;

    backend
        .get_completion(prompt, &tx)
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
                Event::BackendPromptResponse(msg) => responses.push(msg),
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
        .set_models(vec!["gpt-3.5-turbo".to_string(), "gpt-4".to_string()])
        .await;

    backend
        .set_current_model("gpt-3.5-turbo")
        .await
        .expect("Failed to set current model");
    backend
}
