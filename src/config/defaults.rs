use super::constants::*;

pub fn hello_message() -> Option<String> {
    Some(HELLO_MESSAGE.to_string())
}

pub fn max_context_length() -> usize {
    MAX_CONTEXT_LENGTH
}

pub fn max_convo_length() -> usize {
    MAX_CONVO_LENGTH
}

pub fn keep_n_messages() -> usize {
    KEEP_N_MESSAGES
}

pub fn log_level() -> Option<String> {
    Some("info".to_string())
}

pub fn log_file_path() -> String {
    LOG_FILE_PATH.to_string()
}

pub fn bubble_width_percent() -> usize {
    BUBBLE_WIDTH_PERCENT
}

pub fn default_option_true() -> Option<bool> {
    Some(true)
}
