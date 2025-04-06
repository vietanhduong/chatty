#[cfg(test)]
#[path = "bubble_test.rs"]
mod tests;

use crate::{config, models::Message};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use syntect::highlighting::Theme;
use unicode_width::UnicodeWidthStr;

use super::utils;

pub const DEFAULT_PADDING: usize = 8;
pub const DEFAULT_BORDER_ELEMENTS_LEN: usize = 5;
pub const DEFAULT_OUTER_PADDING_PERCENTAGE: f32 = 0.04;

pub struct Bubble<'a> {
    message: &'a Message,
    max_width: usize,

    // Settings
    padding: usize,
    boder_elements_length: usize,
    outer_padding_percentage: f32,
}

impl<'a> Bubble<'_> {
    pub fn new(message: &'a Message, max_width: usize) -> Bubble<'a> {
        Bubble {
            message,
            max_width,

            // Settings
            // Unicode character border + padding
            padding: DEFAULT_PADDING,

            // left boder + left padding + (text, not counted) + right padding
            // + right border + scrollbar
            boder_elements_length: DEFAULT_BORDER_ELEMENTS_LEN,
            outer_padding_percentage: DEFAULT_OUTER_PADDING_PERCENTAGE,
        }
    }

    pub fn with_padding(mut self, padding: usize) -> Self {
        self.padding = padding;
        self
    }

    pub fn with_boder_elements_length(mut self, boder_elements_length: usize) -> Self {
        self.boder_elements_length = boder_elements_length;
        self
    }

    pub fn with_outer_padding_percentage(mut self, outer_padding_percentage: f32) -> Self {
        self.outer_padding_percentage = outer_padding_percentage;
        self
    }

    pub fn padding(&self) -> usize {
        self.padding
    }

    pub fn boder_elements_length(&self) -> usize {
        self.boder_elements_length
    }

    pub fn outer_padding_percentage(&self) -> f32 {
        self.outer_padding_percentage
    }

    pub fn as_lines(&mut self, theme: &'a Theme) -> Vec<Line<'a>> {
        let max_line_len = self.get_max_line_length();

        let lines = utils::build_message_lines(self.message.text(), max_line_len, theme, |line| {
            self.format_spans(line.spans, max_line_len)
        });

        self.wrap_lines_in_bubble(lines, max_line_len)
    }

    fn wrap_lines_in_bubble(&self, lines: Vec<Line<'a>>, max_line_len: usize) -> Vec<Line<'a>> {
        // Replace top bar ─ with the issuer string
        let issuer = self.message.issuer_str();
        let top_bar = format!(
            "╭─ {} {}╮",
            issuer,
            ["─"].repeat(max_line_len - issuer.width() - 1).join("")
        );

        // Replace bottom bar ─ with the date
        let date = self
            .message
            .created_at()
            .with_timezone(&chrono::Local)
            .format("%H:%M %m/%d");
        let bottom_bar = format!(
            "╰─ {} {}╯",
            date,
            ["─"]
                .repeat(max_line_len - date.to_string().width() - 1)
                .join("")
        );
        let bar_padding =
            utils::repeat_from_substactions(" ", vec![self.max_width, max_line_len, self.padding]);

        if self.message.is_system() {
            let mut res = vec![self.highlighted_line(format!("{top_bar}{bar_padding}"))];
            res.extend(lines);
            res.push(self.highlighted_line(format!("{bottom_bar}{bar_padding}")));
            return res;
        }

        let mut res = vec![self.highlighted_line(format!("{bar_padding}{top_bar}"))];
        res.extend(lines);
        res.push(self.highlighted_line(format!("{bar_padding}{bottom_bar}")));
        res
    }

    fn get_max_line_length(&self) -> usize {
        let min_bubble_padding_length =
            ((self.max_width as f32 * self.outer_padding_percentage).ceil()) as usize;

        let line_boder_width = self.boder_elements_length + min_bubble_padding_length;
        let mut max_line_len = self
            .message
            .text()
            .lines()
            .map(|line| line.width())
            .max()
            .unwrap_or_default();

        if max_line_len > (self.max_width - line_boder_width) {
            max_line_len = self.max_width - line_boder_width;
        }

        let issuer = &self.message.issuer_str();
        // 2 Padding space
        if issuer.width() + 2 > max_line_len {
            max_line_len = issuer.width() + 2;
        }

        // date format
        let date = &self
            .message
            .created_at()
            .with_timezone(&chrono::Local)
            .format("%H:%M %m/%d");

        if date.to_string().width() + 2 > max_line_len {
            max_line_len = date.to_string().width() + 2;
        }

        let max_width_percent =
            config::instance().general.get_bubble_width_percent() as f32 / 100.0;

        if max_line_len as f32 > max_width_percent * self.max_width as f32 {
            max_line_len = (self.max_width as f32 * max_width_percent).ceil() as usize;
        }

        max_line_len
    }

    fn format_spans(&self, mut spans: Vec<Span<'a>>, max_line_len: usize) -> Line<'a> {
        let line_str_len: usize = spans.iter().map(|e| e.content.width()).sum();
        let fill = utils::repeat_from_substactions(" ", vec![max_line_len, line_str_len]);
        let formatted_line_len = line_str_len + fill.len() + self.padding;

        let mut wrapped_spans = vec![self.highlighted_span("│ ".to_string())];
        wrapped_spans.append(&mut spans);
        wrapped_spans.push(self.highlighted_span(format!("{fill} │")));

        let outer_padding =
            utils::repeat_from_substactions(" ", vec![self.max_width, formatted_line_len]);

        if self.message.is_system() {
            // Left alignment
            wrapped_spans.push(Span::from(outer_padding));
            return Line::from(wrapped_spans);
        }

        let mut line_spans = vec![Span::from(outer_padding)];
        line_spans.extend(wrapped_spans);

        Line::from(line_spans)
    }

    fn highlighted_span(&self, text: String) -> Span<'a> {
        let color = if self.message.is_system() {
            Color::Rgb(255, 140, 105)
        } else {
            Color::Rgb(64, 224, 208)
        };
        Span::styled(
            text,
            Style {
                fg: Some(color),
                ..Style::default()
            },
        )
    }

    fn highlighted_line(&self, text: String) -> Line<'a> {
        Line::from(self.highlighted_span(text))
    }
}
