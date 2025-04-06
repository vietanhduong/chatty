use std::sync::Arc;

use mockito::Matcher;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::*;

#[tokio::test]
async fn test_list_models() {
    let body = serde_json::to_string(&ModelListResponse {
        models: vec![
            ModelResponse {
                name: "models/gemini-2.0-flash".to_string(),
                supported_generation_methods: vec!["generateContent".to_string()],
            },
            ModelResponse {
                name: "models/gemini-2.0-flash-lite".to_string(),
                supported_generation_methods: vec!["generateContent".to_string()],
            },
            ModelResponse {
                name: "models/gemini-1.5-flash".to_string(),
                supported_generation_methods: vec!["generateContent".to_string()],
            },
            ModelResponse {
                name: "models/gemini-2.1-flash".to_string(),
                supported_generation_methods: vec!["chat".to_string()],
            },
        ],
    });

    let mut server = mockito::Server::new_async().await;

    let models_handler = server
        .mock("GET", "/models")
        .with_status(200)
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "key".into(),
            "test_token".into(),
        )]))
        .with_body(body.unwrap())
        .expect_at_most(1)
        .create();

    let backend = Gemini::default()
        .with_endpoint(&server.url())
        .with_api_key("test_token")
        .with_want_models(vec![
            "gemini-2.0-flash".to_string(),
            "model/gemini-2.0-flash-lite".to_string(),
        ]);

    let res = backend.list_models().await.expect("Failed to list models");

    assert_eq!(res.len(), 2);
    assert_eq!(res[0].id(), "gemini-2.0-flash");
    assert_eq!(res[1].id(), "gemini-2.0-flash-lite");

    models_handler.assert();
}

#[tokio::test]
async fn test_get_completion() {
    let body = std::fs::read_to_string("./testdata/gemini_response.json")
        .expect("Failed to read test data");

    let prompt = BackendPrompt::new("Hello").with_model("gemini-2.0-flash");

    let mut server = mockito::Server::new_async().await;
    let completion_handler = server
        .mock("POST", "/models/gemini-2.0-flash:streamGenerateContent")
        .with_status(200)
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "key".into(),
            "test_token".into(),
        )]))
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

    let events = collect_responses(&mut rx, time::Duration::from_secs(5), 4)
        .await
        .expect("Failed to collect events");
    assert_eq!(events.len(), 4);

    let text = events
        .iter()
        .map(|e| e.text.clone())
        .collect::<Vec<_>>()
        .join("");

    assert_eq!(text, "This is a test");
    let last = events.last().unwrap();
    assert_eq!(last.text, "test");
    assert_eq!(last.done, true);
    assert_eq!(last.model, "gemini-2.0-flash");
    assert_eq!(last.init_conversation, true);
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

async fn setup_backend(url: String) -> Gemini {
    let backend = Gemini::default()
        .with_endpoint(&url)
        .with_api_key("test_token")
        .with_want_models(vec![
            "gemini-2.0-flash".to_string(),
            "gemini-2.0-flash-lite".to_string(),
        ]);
    backend
}
