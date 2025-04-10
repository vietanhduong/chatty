use std::collections::HashMap;

use crate::models::Message;
use ratatui::{buffer::Buffer, layout::Rect, text::Line};
use syntect::highlighting::Theme;

use super::bubble::Bubble;

struct CacheEntry<'a> {
    message_id: String,
    text_len: usize,
    lines: Vec<Line<'a>>,
}

pub struct BubbleList<'a> {
    theme: &'a Theme,
    cache: HashMap<usize, CacheEntry<'a>>,
    line_width: usize,
    line_len: usize,
}

impl<'a> BubbleList<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            cache: HashMap::new(),
            line_len: 0,
            line_width: 0,
        }
    }

    pub fn remove_message(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.cache.retain(|_, entry| entry.message_id != id);
        self.line_len = self.cache.values().map(|entry| entry.lines.len()).sum();
    }

    pub fn remove_message_by_index(&mut self, index: usize) {
        if let Some(entry) = self.cache.remove(&index) {
            self.line_len -= entry.lines.len();
        }
    }

    pub fn set_messages(&mut self, messages: &[Message], line_width: usize) {
        if self.line_width != line_width {
            self.cache.clear();
            self.line_width = line_width;
        }

        self.line_len = messages
            .iter()
            .enumerate()
            .map(|(i, message)| {
                if self.cache.contains_key(&i) {
                    let cache_entry = self.cache.get(&i).unwrap();
                    if i < (messages.len() - 1) || message.text().len() == cache_entry.text_len {
                        return cache_entry.lines.len();
                    }
                }

                let bubble_lines = Bubble::new(message, line_width).as_lines(self.theme);
                let bubble_lines_len = bubble_lines.len();

                self.cache.insert(
                    i,
                    CacheEntry {
                        message_id: message.id().to_string(),
                        text_len: message.text().len(),
                        lines: bubble_lines,
                    },
                );

                bubble_lines_len
            })
            .sum();
    }

    pub fn len(&self) -> usize {
        self.line_len
    }

    pub fn is_empty(&self) -> bool {
        self.line_len == 0
    }

    pub fn render(&self, rect: Rect, buf: &mut Buffer, scroll_index: u16) {
        let mut cache_keys: Vec<usize> = self.cache.keys().cloned().collect();
        cache_keys.sort();

        let mut line_index = 0;
        let mut should_break = false;
        for cache_key in cache_keys {
            for line in self.cache.get(&cache_key).unwrap().lines.as_slice() {
                if line_index < scroll_index {
                    line_index += 1;
                    continue;
                }
                if (line_index - scroll_index) >= rect.height {
                    should_break = true;
                    break;
                }

                buf.set_line(0, line_index - scroll_index, line, rect.width);
                line_index += 1;
            }

            if should_break {
                break;
            }
        }
    }
}
