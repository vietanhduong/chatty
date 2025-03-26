pub(crate) const MIGRATION: &str = r#"
    CREATE TABLE IF NOT EXISTS conversations (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS messages (
        id TEXT NOT NULL,
        conversation_id TEXT NOT NULL,
        text TEXT NOT NULL,
        issuer TEXT NOT NULL,
        system INTEGER NOT NULL,
        token_count INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        PRIMARY KEY (id, conversation_id),
        FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
    );

    CREATE TABLE IF NOT EXISTS contexts (
        id TEXT NOT NULL,
        conversation_id TEXT NOT NULL,
        last_message_id TEXT NOT NULL,
        content TEXT NOT NULL,
        token_count INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        PRIMARY KEY (id, conversation_id, last_message_id),
        FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
        FOREIGN KEY (last_message_id, conversation_id) REFERENCES messages(id, conversation_id) ON DELETE CASCADE
    );
"#;
