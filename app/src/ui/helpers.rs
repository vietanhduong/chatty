use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
};
use syntect::{easy::HighlightLines, highlighting::Theme};
use unicode_width::UnicodeWidthStr;

use crate::ui::syntaxes::{SYNTAX_SET, Syntaxes};

pub(crate) fn popup_area(area: Rect, percent_width: u16, percent_height: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_height)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_width)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

pub(crate) fn notice_area(area: Rect, percent_width: u16) -> Rect {
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_width)]).flex(Flex::End);
    let [area] = horizontal.areas(area);
    area
}

pub(crate) fn split_to_lines<'a>(
    text: impl Into<Vec<Span<'a>>>,
    max_width: usize,
) -> Vec<Line<'a>> {
    let mut lines = vec![];
    let mut line = vec![];
    let mut line_char_count = 0;
    for word in text.into() {
        if line_char_count + word.content.len() > max_width - 2 {
            lines.push(Line::from(line));
            line = vec![];
            line_char_count = 0;
        }
        line_char_count += word.width() + 1;
        line.push(word);
        line.push(Span::raw(" "));
    }
    if !line.is_empty() {
        lines.push(Line::from(line))
    }
    lines
}

pub(crate) fn build_message_lines<'a, 'b>(
    content: &'b str,
    max_width: usize,
    theme: &'a Theme,
    format_spans: impl Fn(Vec<Span<'a>>) -> Line<'a>,
) -> Vec<Line<'a>> {
    let mut highlight = HighlightLines::new(Syntaxes::get("text"), theme);
    let mut in_codeblock = false;
    let mut lines: Vec<Line> = vec![];

    for line in content.lines() {
        let mut spans = vec![];
        if line.trim().starts_with("```") {
            let lang = line.trim().replace("```", "");
            let syntax = Syntaxes::get(&lang);
            if !in_codeblock {
                highlight = HighlightLines::new(syntax, &theme);
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

        let mut split_spans = vec![];
        let mut line_char_count = 0;

        for span in spans {
            if span.content.width() + line_char_count <= max_width {
                line_char_count += span.content.width();
                split_spans.push(span);
                continue;
            }

            let mut word_set: Vec<&str> = vec![];
            for word in span.content.split(' ') {
                if word.len() + line_char_count > max_width {
                    split_spans.push(Span::styled(word_set.join(" "), span.style));
                    lines.push(format_spans(split_spans));
                    split_spans = vec![];
                    word_set = vec![];
                    line_char_count = 0;
                }

                word_set.push(word);
                line_char_count += word.len() + 1;
            }

            split_spans.push(Span::styled(word_set.join(" "), span.style));
        }

        lines.push(format_spans(split_spans));
    }
    lines
}

pub(crate) fn repeat_from_substactions(text: &str, subs: Vec<usize>) -> String {
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
