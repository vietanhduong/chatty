use std::{collections::BTreeMap, sync::Arc};

use crate::models::Message;
use ratatui::{buffer::Buffer, layout::Rect, text::Line};
use syntect::highlighting::Theme;
use unicode_width::UnicodeWidthStr;

use super::{Content, Selectable, Selection, bubble::Bubble};

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

    pub fn lines(&self) -> &[Arc<Line<'a>>] {
        &self.lines
    }

    pub fn screen_pos_to_line_pos(
        &self,
        x: u16,
        y: u16,
        scroll_index: u16,
    ) -> Option<(usize, usize)> {
        let actual_y = (y + scroll_index) as usize;
        if actual_y >= self.lines.len() {
            return None;
        }
        let line = &self.lines[actual_y];
        if !line.is_selectable() {
            return None;
        }

        let x = x as usize;
        let mut ptr = line.content().width();
        for span in line.spans.iter().rev() {
            let span_width = span.content.width();
            ptr -= span_width;
            if !span.is_selectable() {
                continue;
            }
            if x >= ptr && x <= ptr + span_width {
                return Some((actual_y, x));
            }
            if x > ptr + span_width {
                return Some((actual_y, ptr + span_width));
            }
        }
        None
    }

    pub fn get_visible_lines(&self, height: usize, scroll_index: usize) -> Vec<Arc<Line<'a>>> {
        let visible_lines = self
            .lines
            .iter()
            .skip(scroll_index)
            .take(height)
            .cloned()
            .collect();
        visible_lines
    }

    pub fn render(&self, rect: Rect, buf: &mut Buffer, scroll_index: usize, sel: &Selection) {
        for (i, line) in self
            .get_visible_lines(rect.height as usize, scroll_index)
            .iter()
            .enumerate()
        {
            let mut line = line.as_ref().clone();
            if line.is_selectable() && sel.contains_row(i + scroll_index) {
                line = sel.format_line(line, i + scroll_index);
            }
            buf.set_line(0, i as u16, &line, rect.width);
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
