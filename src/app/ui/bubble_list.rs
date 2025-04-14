use std::{collections::BTreeMap, sync::Arc};

use crate::models::Message;
use ratatui::{buffer::Buffer, layout::Rect, text::Line};
use syntect::highlighting::Theme;

use super::bubble::Bubble;

struct CacheEntry<'a> {
    message_id: String,
    text_len: usize,
    lines: Vec<Arc<Line<'a>>>,
}

pub struct BubbleList<'a> {
    theme: &'a Theme,
    cache: BTreeMap<usize, CacheEntry<'a>>,
    lines: Vec<Arc<Line<'a>>>,
    line_width: usize,
    line_len: usize,
}

impl<'a> BubbleList<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            cache: BTreeMap::new(),
            lines: Vec::new(),
            line_len: 0,
            line_width: 0,
        }
    }

    pub fn remove_message(&mut self, id: impl Into<String>) {
        let id = id.into();
        self.cache.retain(|_, entry| entry.message_id != id);
        self.update_lines();
        self.line_len = self.cache.values().map(|entry| entry.lines.len()).sum();
    }

    pub fn remove_message_by_index(&mut self, index: usize) {
        if let Some(entry) = self.cache.remove(&index) {
            self.update_lines();
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
                        lines: bubble_lines.into_iter().map(Arc::new).collect(),
                    },
                );

                bubble_lines_len
            })
            .sum();
        self.update_lines();
    }

    pub fn len(&self) -> usize {
        self.line_len
    }

    pub fn is_empty(&self) -> bool {
        self.line_len == 0
    }

    pub fn render(&self, rect: Rect, buf: &mut Buffer, scroll_index: u16) {
        let visible_lines = self
            .lines
            .iter()
            .skip(scroll_index as usize)
            .take(rect.height as usize);
        for (i, line) in visible_lines.enumerate() {
            buf.set_line(0, i as u16, line, rect.width);
        }
    }

    fn update_lines(&mut self) {
        self.lines = self
            .cache
            .values()
            .flat_map(|entry| entry.lines.clone())
            .collect();
    }
}
