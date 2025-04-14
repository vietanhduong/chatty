use std::cmp::Ordering;

use ratatui::{
    style::{Color, Stylize},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use super::{Content, Selectable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Index {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Default)]
pub struct Selection {
    start: Option<Index>,
    end: Option<Index>,
}

impl Selection {
    pub fn set_start(&mut self, row: usize, col: usize) {
        self.start = Some(Index { row, col });
    }

    pub fn set_end(&mut self, row: usize, col: usize) {
        if self.start.is_none() {
            self.start = Some(Index { row, col });
            return;
        }
        self.end = Some(Index { row, col });
    }

    pub fn start(&self) -> Option<Index> {
        self.start
    }

    pub fn end(&self) -> Option<Index> {
        self.end
    }

    pub fn clear(&mut self) {
        self.start = None;
        self.end = None;
    }

    pub fn is_empty(&self) -> bool {
        self.start.is_none() && self.end.is_none()
    }

    fn is_reverse(&self) -> bool {
        match (self.start, self.end) {
            (Some(start), Some(end)) => {
                start.row > end.row || start.row == end.row && start.col > end.col
            }
            _ => false,
        }
    }

    pub fn get_bounds(&self) -> Option<(Index, Index)> {
        match (self.start, self.end) {
            (Some(start), Some(end)) => {
                if self.is_reverse() {
                    return Some((end, start));
                }
                Some((start, end))
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn contains(&self, pos: Index) -> bool {
        if let Some((start, end)) = self.get_bounds() {
            let (st_row, st_col) = (start.row, start.col);
            let (en_row, en_col) = (end.row, end.col);

            return match (pos.row, pos.col) {
                (line, _) if line > st_row && line < en_row => true,
                (line, column) if line > st_row && line == en_row => column <= en_col,
                (line, column) if line == st_row && line < en_row => column >= st_col,
                (line, column) if line == st_row && line == en_row => {
                    column <= en_col && column >= st_col
                }
                _ => false,
            };
        }
        false
    }

    #[must_use]
    pub fn contains_row(&self, row_index: usize) -> bool {
        if let Some((start, end)) = self.get_bounds() {
            return row_index >= start.row && row_index <= end.row;
        }
        false
    }

    /// Returns the start and end column of the selection in the given row.
    /// If the selection does not intersect with the row, the function returns None.
    #[must_use]
    pub fn get_selected_columns_in_row(
        &self,
        row_index: usize,
        row_len: usize,
    ) -> Option<(usize, usize)> {
        if let Some((start, end)) = self.get_bounds() {
            let start_col = match start.row.cmp(&row_index) {
                Ordering::Less => 0,
                Ordering::Greater => return None,
                Ordering::Equal => start.col.min(row_len),
            };

            let end_col = match end.row.cmp(&row_index) {
                Ordering::Less => return None,
                Ordering::Greater => row_len,
                Ordering::Equal => end.col.min(row_len),
            };
            return Some((start_col, end_col));
        }
        None
    }

    #[must_use]
    pub fn format_line<'a>(&self, line: Line<'a>, row: usize) -> Line<'a> {
        let Some((start, end)) = self.get_selected_columns_in_row(row, line.content().width())
        else {
            return line;
        };

        let mut ptr = 0;
        let mut ret_line = line.clone();
        ret_line.spans = vec![];
        for span in line.spans.into_iter() {
            let span_width = span.content.width();
            if !span.is_selectable() {
                ret_line.spans.push(span);
                ptr += span_width;
                continue;
            }

            // Check if span is completely outside selection
            if ptr + span_width <= start || ptr >= end {
                ret_line.spans.push(span);
                ptr += span_width;
                continue;
            }

            // If span is completely within selection
            if ptr >= start && ptr + span_width <= end {
                ret_line.spans.push(span.fg(Color::Black).bg(Color::Blue));
                ptr += span_width;
                continue;
            }

            // Handle partial selection
            let content: Vec<char> = span.content.chars().collect();

            // Calculate selection bounds within this span
            let sel_start = start.saturating_sub(ptr);
            let sel_end = (end - ptr).min(span_width);

            // Add prefix if needed (unselected text before selection)
            if ptr < start {
                let prefix_end = sel_start;
                let prefix: String = content[..prefix_end].iter().collect();
                ret_line.spans.push(Span::styled(prefix, span.style));
            }

            // Add selected portion
            if sel_end >= sel_start {
                let selected: String = content[sel_start..sel_end].iter().collect();
                ret_line.spans.push(Span::styled(
                    selected,
                    span.style.fg(Color::Black).bg(Color::Blue),
                ));
            }

            // Add suffix if needed (unselected text after selection)
            if ptr + span_width > end {
                let suffix_start = sel_end;
                let suffix: String = content[suffix_start..].iter().collect();
                ret_line.spans.push(Span::styled(suffix, span.style));
            }

            ptr += span_width;
        }
        ret_line
    }
}
