use super::*;
use crate::models::{BackendKind, StorageConfig};

#[test]
fn test_load_configuration() {
    let config = load_configuration("./testdata/config.toml").expect("failed to load config");

    let log = config.log().unwrap();
    assert_eq!(log.level(), Some("info"));
    let log_filters = log.filters().unwrap_or_default();
    assert_eq!(log_filters.len(), 1);
    assert_eq!(log_filters[0].module(), "backend");

    let log_file = log.file();
    assert!(log_file.is_some());
    assert_eq!(log_file.unwrap().path(), "/var/log/chatty.log");
    assert_eq!(log_file.unwrap().append(), true);

    assert_eq!(config.theme().unwrap().name(), Some("dark"));
    assert_eq!(
        config.theme().unwrap().folder_path(),
        Some("/etc/chatty/theme")
    );

    let backend = config.backend().unwrap();
    assert_eq!(backend.connections().len(), 2);
    assert_eq!(backend.timeout_secs(), Some(60));

    let compression = backend.context_compression();
    assert_eq!(compression.enabled(), true);
    assert_eq!(compression.max_tokens(), 120_000);
    assert_eq!(compression.keep_n_messages(), 10);
    assert_eq!(compression.max_messages(), 100);

    let deepseek = backend
        .connections()
        .iter()
        .find(|c| c.alias() == Some("deepseek"))
        .unwrap();
    assert_eq!(deepseek.enabled(), true);
    assert_eq!(deepseek.alias(), Some("deepseek"));
    assert_eq!(deepseek.kind().to_string(), BackendKind::OpenAI.to_string());
    assert_eq!(deepseek.endpoint(), "https://api.deepseek.com");

    let openai = backend
        .connections()
        .iter()
        .find(|c| c.alias() == Some("openai"))
        .unwrap();
    assert_eq!(openai.alias(), Some("openai"));
    assert_eq!(openai.enabled(), true);
    assert_eq!(openai.kind().to_string(), BackendKind::OpenAI.to_string());
    assert_eq!(openai.endpoint(), "https://api.openai.com");
    assert_eq!(openai.models(), &["gpt-3.5-turbo", "gpt-4"]);

    let model = backend.default_model().unwrap();
    assert_eq!(model, "gpt-3.5-turbo");

    let storage = config.storage().unwrap();

    match storage {
        StorageConfig::Sqlite(sqlite) => {
            assert_eq!(sqlite.path(), Some("/var/lib/chatty/chat.db"));
        }
    }
}

#[test]
fn test_resolve_path() {
    let ret = resolve_path("$TEST_PATH/${USER_PATH}/config.toml").expect("failed to resolve path");
    assert_eq!(ret, "//config.toml");

    let dir = "/tmp/test";
    let user_path = "user_path";
    unsafe {
        std::env::set_var("TEST_PATH", dir);
        std::env::set_var("USER_PATH", user_path);
    }
    let ret = resolve_path("$TEST_PATH/${USER_PATH}/config.toml").expect("failed to resolve path");
    assert_eq!(ret, format!("{dir}/{user_path}/config.toml"));
}
