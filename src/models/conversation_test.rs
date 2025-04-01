use super::*;

#[test]
fn test_filter_issuer() {
    let msg = Message::new_system("system", "Hello, world!");
    assert_eq!(filter_issuer(None, &msg), true);

    let issuer = Issuer::system();
    assert_eq!(filter_issuer(Some(&issuer), &msg), true);
    let issuer = Issuer::system_with_name("system");
    assert_eq!(filter_issuer(Some(&issuer), &msg), true);
    let issuer = Issuer::system_with_name("test");
    assert_eq!(filter_issuer(Some(&issuer), &msg), false);
    let issuer = Issuer::user();
    assert_eq!(filter_issuer(Some(&issuer), &msg), false);
    let msg = Message::new_user("user", "Hello, world!");
    assert_eq!(filter_issuer(Some(&issuer), &msg), true);
    let issuer = Issuer::user_with_name("user");
    assert_eq!(filter_issuer(Some(&issuer), &msg), true);
    let issuer = Issuer::user_with_name("test");
    assert_eq!(filter_issuer(Some(&issuer), &msg), false);
}

#[test]
fn test_conversation_build_context() {
    Configuration::init(Configuration::default()).expect("failed to init default config");

    let mut convo = Conversation::new_hello();
    let context = convo.build_context();
    assert_eq!(context.len(), 0);

    convo.append_message(Message::new_user("user", "Hello, world!"));
    convo.append_message(Message::new_system("system", "Hello, user!"));
    convo.append_message(Message::new_user("user", "How are you?"));
    convo.append_message(
        Message::new_system("system", "I am fine, thank you!").with_id("checkpoint"),
    );
    convo.append_message(Message::new_user("user", "What about you?"));
    convo.append_message(Message::new_system("system", "I am fine too!"));
    convo.append_message(Message::new_user("user", "Ok"));

    let context = convo.build_context();
    assert_eq!(context.len(), 6);
    assert_eq!(context[5].is_system(), true);
    assert_eq!(context[5].text(), "I am fine too!");

    convo.append_context(Context::new("checkpoint").with_content("This is a checkpoint"));

    let context = convo.build_context();
    assert_eq!(context.len(), 3);
    assert_eq!(context[0].text(), "This is a checkpoint");
    assert_eq!(context[0].is_context(), true);
    assert_eq!(context[1].text(), "What about you?");
    assert_eq!(context[1].is_context(), false);
    assert_eq!(context[2].text(), "I am fine too!");
    assert_eq!(context[2].is_context(), false);
}

#[test]
pub fn test_conversation_last_message_of() {
    let mut convo = Conversation::new_hello();

    convo.append_message(Message::new_user("user", "Hello, world!"));
    convo.append_message(Message::new_system("system", "Hello, user!"));
    convo.append_message(Message::new_user("user", "How are you?"));
    convo.append_message(
        Message::new_system("system", "I am fine, thank you!").with_id("checkpoint"),
    );
    convo.append_message(Message::new_user("user", "What about you?"));
    convo.append_message(Message::new_system("system", "I am fine too!"));
    convo.append_message(Message::new_user("user", "Ok"));

    let msg = convo.last_message_of(None).unwrap();
    assert_eq!(msg.text(), "Ok");
    assert_eq!(msg.issuer_str(), "user");

    let msg = convo.last_message_of(Some(Issuer::system())).unwrap();
    assert_eq!(msg.text(), "I am fine too!");
    assert_eq!(msg.issuer_str(), "system");

    let msg = convo.last_message_of(Some(Issuer::user())).unwrap();
    assert_eq!(msg.text(), "Ok");
    assert_eq!(msg.issuer_str(), "user");
}

#[test]
pub fn test_conversation_token_count() {
    Configuration::init(Configuration::default()).expect("failed to init default config");

    let mut convo = Conversation::new_hello();

    convo.append_message(
        Message::new_user("user", "Hello, world!")
            .with_id("1")
            .with_token_count(3),
    );
    convo.append_message(
        Message::new_system("system", "Hello, user!")
            .with_id("2")
            .with_token_count(3),
    );
    convo.append_message(
        Message::new_user("user", "How are you?")
            .with_id("3")
            .with_token_count(3),
    );

    convo.append_message(
        Message::new_system("system", "I am fine, thank you!")
            .with_id("checkpoint")
            .with_token_count(5),
    );

    convo.append_message(
        Message::new_user("user", "What about you?")
            .with_id("4")
            .with_token_count(3),
    );
    convo.append_message(
        Message::new_system("system", "I am fine too!")
            .with_id("5")
            .with_token_count(4),
    );
    convo.append_message(
        Message::new_user("user", "Ok")
            .with_id("6")
            .with_token_count(1),
    );

    let token_count = convo.token_count();
    assert_eq!(token_count, 22);

    convo.append_context(
        Context::new("checkpoint")
            .with_content("This is a checkpoint")
            .with_token_count(4),
    );

    let token_count = convo.token_count();
    assert_eq!(token_count, 12);
}
