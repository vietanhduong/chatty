/// Max context tokens before trigger compression
pub const MAX_CONTEXT_LENGTH: usize = 64 * 1024; // 64k tokens

/// Max conversation legnth before trigger compression
pub const MAX_CONVO_LENGTH: usize = 50; // 50 messages

/// Keep N lastest messages and compress the rest
pub const KEEP_N_MEESAGES: usize = 5; // Keep last 5 messages

pub const HELLO_MESSAGE: &str = "Hello! How can I help you? 😊";

pub const LOG_FILE_PATH: &str = "/tmp/chatty.log";

pub const BUBBLE_WIDTH_PERCENT: usize = 60; // 60% of the screen width

pub const MAX_BUBBLE_WIDTH_PERCENT: usize = 95; // 80% of the screen width

pub const MIN_BUBBLE_WIDTH_PERCENT: usize = 50; // 20% of the screen width
