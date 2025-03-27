#[cfg(test)]
#[path = "utils_test.rs"]
mod tests;

use crate::{config::Configuration, models::Message};

pub(crate) fn context_truncation(context: &mut Vec<Message>, max_output_tokens: usize) {
    if !Configuration::instance().context.truncation.enabled || max_output_tokens == 0 {
        return;
    }

    let max_tokens = Configuration::instance().context.truncation.max_tokens;
    let mut current_tokens = context.iter().map(|msg| msg.token_count()).sum::<usize>();
    if current_tokens + max_output_tokens <= max_tokens {
        return;
    }

    let mut idx = 0;
    while current_tokens + max_output_tokens > max_tokens {
        if context.len() < 2 {
            break;
        }

        if context[idx].is_context() {
            idx += 1;
            continue;
        }
        let msg = context.remove(idx);
        current_tokens -= msg.token_count();
    }

    if !context.last().unwrap().is_system() {
        context.pop();
    }
}
