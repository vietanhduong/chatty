use openai_models::Message;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use syntect::{easy::HighlightLines, highlighting::ThemeSet};

use crate::syntaxes::{SYNTAX_SET, Syntaxes};

pub struct Bubble<'a> {
    message: &'a Message,
    max_width: usize,
    codeblock_counter: usize,

    // Settings
    padding: usize,
    boder_elements_length: usize,
    outer_padding_percentage: f32,
}

impl<'a> Bubble<'_> {
    pub fn new(message: &'a Message, max_width: usize, codeblock_counter: usize) -> Bubble<'a> {
        Bubble {
            message,
            max_width,
            codeblock_counter,

            // Settings
            // Unicode character border + padding
            padding: 8,

            // left boder + left padding + (text, not counted) + right padding
            // + right border + scrollbar
            boder_elements_length: 5,
            outer_padding_percentage: 0.04,
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

    pub fn as_lines(&mut self) -> Vec<Line<'a>> {
        let ts = ThemeSet::load_defaults();
        let theme = ts.themes.get("base16-ocean.dark").unwrap();
        let mut highlight = HighlightLines::new(Syntaxes::get("text"), &theme);
        let mut in_codeblock = false;
        let mut lines: Vec<Line> = vec![];

        let max_line_length = self.get_max_line_length();

        for line in self.message.text().lines() {
            let mut spans = vec![];
            if line.trim().starts_with("```") {
                let lang = line.trim().replace("```", "");
                let syntax = Syntaxes::get(&lang);
                if !in_codeblock {
                    highlight = HighlightLines::new(syntax, &theme);
                    in_codeblock = true;
                    self.codeblock_counter += 1;
                    spans = vec![
                        Span::from(line.to_owned()),
                        Span::styled(
                            format!(" ({})", self.codeblock_counter),
                            Style {
                                fg: Some(Color::White),
                                ..Style::default()
                            },
                        ),
                    ];
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
                if span.content.len() + line_char_count <= max_line_length {
                    line_char_count += span.content.len();
                    split_spans.push(span);
                    continue;
                }

                let mut word_set: Vec<&str> = vec![];
                for word in span.content.split(' ') {
                    if word.len() + line_char_count > max_line_length {
                        split_spans.push(Span::styled(word_set.join(" "), span.style));
                        lines.push(self.spans_to_line(split_spans, max_line_length));
                        split_spans = vec![];
                        word_set = vec![];
                        line_char_count = 0;
                    }

                    word_set.push(word);
                    line_char_count += word.len() + 1;
                }

                split_spans.push(Span::styled(word_set.join(" "), span.style));
            }

            lines.push(self.spans_to_line(split_spans, max_line_length));
        }

        self.wrap_lines_in_bubble(lines, max_line_length)
    }

    fn wrap_lines_in_bubble(&self, lines: Vec<Line<'a>>, max_line_len: usize) -> Vec<Line<'a>> {
        let inner_bar = ["─"].repeat(max_line_len + 2).join("");
        let top_bar = format!("╭{inner_bar}╮");
        let bottom_bar = format!("╰{inner_bar}╯");
        let bar_padding =
            repeat_from_substactions(" ", vec![self.max_width, max_line_len, self.padding]);

        if self.message.system() {
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
            .map(|line| line.len())
            .max()
            .unwrap();

        if max_line_len > (self.max_width - line_boder_width) {
            max_line_len = self.max_width - line_boder_width;
        }
        if max_line_len as f32 > 0.75 * self.max_width as f32 {
            max_line_len = (self.max_width as f32 * 0.75).ceil() as usize;
        }

        max_line_len
    }

    fn spans_to_line(&self, mut spans: Vec<Span<'a>>, max_line_len: usize) -> Line<'a> {
        let line_str_len: usize = spans.iter().map(|e| return e.content.len()).sum();
        let fill = repeat_from_substactions(" ", vec![max_line_len, line_str_len]);
        let formatted_line_len = line_str_len + fill.len() + self.padding;

        let mut wrapped_spans = vec![self.highlighted_span("│ ".to_string())];
        wrapped_spans.append(&mut spans);
        wrapped_spans.push(self.highlighted_span(format!("{fill} │")));

        let outer_padding = repeat_from_substactions(" ", vec![self.max_width, formatted_line_len]);

        if self.message.system() {
            // Left alignment
            wrapped_spans.push(Span::from(outer_padding));
            return Line::from(wrapped_spans);
        }

        let mut line_spans = vec![Span::from(outer_padding)];
        line_spans.extend(wrapped_spans);

        Line::from(line_spans)
    }

    fn highlighted_span(&self, text: String) -> Span<'a> {
        Span::from(text)
    }

    fn highlighted_line(&self, text: String) -> Line<'a> {
        Line::from(self.highlighted_span(text))
    }
}

fn repeat_from_substactions(text: &str, subs: Vec<usize>) -> String {
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
