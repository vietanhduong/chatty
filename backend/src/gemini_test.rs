use std::sync::Arc;

use mockito::Matcher;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::*;

impl Gemini {
    async fn set_models(&self, models: Vec<String>) {
        let mut write = self.cache_models.write().await;
        *write = models;
    }
}

#[tokio::test]
async fn test_list_models() {
    let body = serde_json::to_string(&ModelListResponse {
        models: vec![
            Model {
                name: "models/gemini-2.0-flash".to_string(),
                supported_generation_methods: vec!["generateContent".to_string()],
            },
            Model {
                name: "models/gemini-2.0-flash-lite".to_string(),
                supported_generation_methods: vec!["generateContent".to_string()],
            },
            Model {
                name: "models/gemini-1.5-flash".to_string(),
                supported_generation_methods: vec!["generateContent".to_string()],
            },
            Model {
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

    let res = backend
        .list_models(false)
        .await
        .expect("Failed to list models");

    assert_eq!(res.len(), 2);
    assert_eq!(res[0], "gemini-2.0-flash");
    assert_eq!(res[1], "gemini-2.0-flash-lite");

    models_handler.assert();

    let res = backend
        .list_models(false)
        .await
        .expect("Failed to list models");

    assert_eq!(res.len(), 2);
    assert_eq!(res[0], "gemini-2.0-flash");
    assert_eq!(res[1], "gemini-2.0-flash-lite");
    // Hit cache, no request to server
    models_handler.assert();
}

#[tokio::test]
async fn test_set_current_model() {
    let backend = Gemini::default();

    backend
        .set_models(vec![
            "gemini-2.0-flash".to_string(),
            "gemini-2.0-flash-lite".to_string(),
        ])
        .await;

    backend
        .set_current_model("gemini-2.0-flash")
        .await
        .expect("Failed to set current model");

    assert_eq!(
        backend.current_model().await,
        Some("gemini-2.0-flash".to_string())
    );

    let err = backend.set_current_model("o1-mini").await.unwrap_err();
    assert!(err.to_string().contains("model o1-mini not available"));
}

#[tokio::test]
async fn test_get_completion() {
    let body = r#"
{
  "contents": [
    {
      "parts": [
        {
          "text": "How "
        }
      ]
    },
    {
      "parts": [
        {
          "text": "can "
        }
      ]
    },
    {
      "parts": [
        {
          "text": "I "
        }
      ]
    },
    {
      "parts": [
        {
          "text": "help "
        }
      ]
    },
    {
      "parts": [
        {
          "text": "you?"
        }
      ]
    },
    {
      "parts": [
        {
          "text": ""
        }
      ]
    }
  ]
}
    "#;

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

    let events = collect_responses(&mut rx, time::Duration::from_secs(5), 6)
        .await
        .expect("Failed to collect events");
    assert_eq!(events.len(), 6);

    let text = events
        .iter()
        .map(|e| e.text.clone())
        .collect::<Vec<_>>()
        .join("");

    assert_eq!(text, "How can I help you?");
    let last = events.last().unwrap();
    assert_eq!(last.text, "");
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

async fn setup_backend(url: String) -> Gemini {
    let backend = Gemini::default()
        .with_endpoint(&url)
        .with_api_key("test_token")
        .with_want_models(vec![
            "gemini-2.0-flash".to_string(),
            "gemini-2.0-flash-lite".to_string(),
        ]);

    backend
        .set_models(vec![
            "gemini-2.0-flash".to_string(),
            "gemini-2.0-flash-lite".to_string(),
        ])
        .await;

    backend
        .set_current_model("gemini-2.0-flash")
        .await
        .expect("Failed to set current model");
    backend
}
