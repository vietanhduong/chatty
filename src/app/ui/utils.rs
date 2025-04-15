#[cfg(test)]
#[path = "utils_test.rs"]
mod tests;

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
};
use ratatui_macros::span;
use syntect::{easy::HighlightLines, highlighting::Theme};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::ui::syntaxes::{SYNTAX_SET, Syntaxes},
    config,
};

use super::Selectable;

pub fn popup_area(area: Rect, percent_width: u16, percent_height: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_height)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_width)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

pub fn notice_area(area: Rect, percent_width: u16) -> Rect {
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_width)]).flex(Flex::End);
    let [area] = horizontal.areas(area);
    area
}

pub fn split_to_lines<'a>(text: impl Into<Line<'a>>, max_width: usize) -> Vec<Line<'a>> {
    let mut lines = vec![];
    let mut line = vec![];
    let mut line_char_count = 0;
    let spans = split_spans(text);

    let wrapper_char = if config::instance()
        .general
        .show_wrapped_indicator
        .unwrap_or_default()
    {
        1
    } else {
        0
    };

    for word in spans {
        if line_char_count + word.content.width() + wrapper_char > max_width && !line.is_empty() {
            line.push(wrapper_span());
            lines.push(Line::from(line));
            line = vec![];
            line_char_count = 0;
        }
        line_char_count += word.width();
        line.push(word);
    }
    if !line.is_empty() {
        lines.push(Line::from(line));
    }
    lines
}

fn split_spans<'a>(input: impl Into<Line<'a>>) -> Vec<Span<'a>> {
    let mut spans = vec![];
    input.into().spans.into_iter().for_each(|item| {
        spans.extend(split_span_by_space(item));
    });
    spans
}

fn split_span_by_space(span: Span) -> Vec<Span> {
    let mut spans = vec![];
    let s = span.content.to_string();
    let mut in_word = false;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        if c == ' ' {
            if in_word {
                spans.push(Span::styled(s[start..i].to_string(), span.style));
                in_word = false;
            }
            let space_end = i + c.len_utf8();
            spans.push(Span::styled(s[i..space_end].to_string(), span.style));
            start = space_end;
        } else if !in_word {
            start = i;
            in_word = true;
        }
    }
    if in_word {
        spans.push(Span::styled(s[start..].to_string(), span.style));
    }
    spans
        .into_iter()
        .filter(|s| s.content.width() > 0)
        .collect()
}

pub fn build_message_lines<'a, 'b, F>(
    content: &'b str,
    max_width: usize,
    theme: &'a Theme,
    format_spans: F,
) -> Vec<Line<'a>>
where
    F: Fn(Line<'a>) -> Line<'a>,
{
    let mut highlight = HighlightLines::new(Syntaxes::get("text"), theme);
    let mut in_codeblock = false;
    let mut lines: Vec<Line> = vec![];

    for line in content.lines() {
        let mut spans = vec![];
        if line.trim().starts_with("```") {
            let lang = line.trim().replace("```", "");
            let syntax = Syntaxes::get(&lang);
            if !in_codeblock {
                highlight = HighlightLines::new(syntax, theme);
                in_codeblock = true;
                spans = vec![Span::from(line.to_owned())];
            } else {
                in_codeblock = false
            }
        } else if in_codeblock {
            let line_nl = format!("{}\n", line);
            let highlighted = highlight.highlight_line(&line_nl, &SYNTAX_SET).unwrap();
            spans = highlighted
                .iter()
                .enumerate()
                .map(|(i, segment)| {
                    let (style, content) = segment;
                    let mut text = content.to_string();
                    if i == highlighted.len() - 1 {
                        text = text.trim_end().to_string();
                    }

                    Span::styled(
                        text,
                        Style {
                            fg: Syntaxes::translate_colour(style.foreground),
                            ..Style::default()
                        },
                    )
                })
                .collect();
        }

        if spans.is_empty() {
            spans = vec![Span::styled(line.to_owned(), Style::default())];
        }

        lines.extend(
            split_to_lines(spans, max_width)
                .into_iter()
                .map(&format_spans)
                .collect::<Vec<_>>(),
        );
    }
    lines
}

pub fn repeat_from_substactions(text: &str, subs: Vec<usize>) -> String {
    let count = subs
        .into_iter()
        .map(|e| i32::try_from(e).unwrap())
        .reduce(|a, b| a - b)
        .unwrap();

    if count <= 0 {
        return String::new();
    }

    [text].repeat(count.try_into().unwrap()).join("")
}

pub fn wrapper_span<'a>() -> Span<'a> {
    if config::instance()
        .general
        .show_wrapped_indicator
        .unwrap_or_default()
    {
        return span!("↵").dim().italic().unselectable();
    }
    span!("")
        .fg(Color::Rgb(0, 0, 0))
        .bg(Color::Rgb(0, 0, 0))
        .unselectable()
}

pub fn is_wrapper_span(span: &Span) -> bool {
    let show = config::instance()
        .general
        .show_wrapped_indicator
        .unwrap_or_default();

    if show {
        span.content == "↵"
            && span.style.add_modifier.contains(Modifier::DIM)
            && span.style.add_modifier.contains(Modifier::ITALIC)
            && !span.is_selectable()
    } else {
        span.content.is_empty()
            && span.style.fg == Some(Color::Rgb(0, 0, 0))
            && span.style.bg == Some(Color::Rgb(0, 0, 0))
            && !span.is_selectable()
    }
}

pub(super) fn spans_to_text(spans: &[Span<'_>]) -> String {
    spans
        .iter()
        .filter(|s| s.is_selectable())
        .map(|s| s.content.to_string())
        .collect::<Vec<String>>()
        .join("")
}
