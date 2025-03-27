use super::*;

#[test]
fn test_context_truncation() {
    let mut config = Configuration::default();
    config.context.truncation.enabled = true;
    config.context.truncation.max_tokens = 15;

    Configuration::init(config).expect("failed to init default config");

    let mut context = vec![
        Message::new_user("user", "Hello, world!").with_token_count(2),
        Message::new_system("system", "Hello, user!").with_token_count(2),
        Message::new_user("user", "How are you?").with_token_count(3),
        Message::new_system("system", "I am fine, thank you!").with_token_count(5),
        Message::new_user("user", "What about you?").with_token_count(3),
        Message::new_system("system", "I am fine too!").with_token_count(5),
        Message::new_user("user", "No, i'm not ok").with_token_count(2),
        Message::new_system("system", "urmom").with_token_count(1),
    ];

    let mut context_2 = context.clone();
    context_2.push(Message::new_user("user", "Ok").with_token_count(1));

    context_truncation(&mut context, 5);
    assert_eq!(context.len(), 3);
    assert_eq!(context[0].text(), "I am fine too!");
    assert_eq!(context[1].text(), "No, i'm not ok");
    assert_eq!(context[2].text(), "urmom");

    context_truncation(&mut context_2, 5);
    assert_eq!(context_2.len(), 3);
    assert_eq!(context[0].text(), "I am fine too!");
    assert_eq!(context[1].text(), "No, i'm not ok");
    assert_eq!(context[2].text(), "urmom");
}
