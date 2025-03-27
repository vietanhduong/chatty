/// Max context tokens before trigger compression
pub const MAX_CONTEXT_LENGTH: usize = 64 * 1024; // 64k tokens

/// Max conversation legnth before trigger compression
pub const MAX_CONVO_LENGTH: usize = 50; // 50 messages

/// Keep N lastest messages and compress the rest
pub const KEEP_N_MEESAGES: usize = 5; // Keep last 5 messages

pub const MAX_OUTPUT_TOKENS: usize = 4096; // 4k tokens

pub const HELLO_MESSAGE: &str = "Hello! How can I help you? 😊";
