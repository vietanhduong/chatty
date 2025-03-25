use std::time::{self, Duration};

use chatty_models::NoticeMessage;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{List, ListItem},
};
use unicode_width::UnicodeWidthStr;

use super::helpers;

struct MessageWrapper {
    value: NoticeMessage,
    created_at: chrono::DateTime<chrono::Utc>,
}

pub struct Notice {
    notices: Vec<MessageWrapper>,
    display_duration: time::Duration,
}

impl Notice {
    pub fn new(display_duration: time::Duration) -> Notice {
        Notice {
            display_duration,
            ..Default::default()
        }
    }

    pub fn add_message(&mut self, msg: NoticeMessage) {
        let now = chrono::Utc::now();
        self.notices.push(MessageWrapper {
            value: msg,
            created_at: now,
        });
    }

    pub fn info(&mut self, msg: impl Into<String>) {
        self.add_message(NoticeMessage::info(msg))
    }

    pub fn warning(&mut self, msg: impl Into<String>) {
        self.add_message(NoticeMessage::warning(msg))
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.add_message(NoticeMessage::error(msg))
    }

    pub fn clear(&mut self) {
        self.notices.clear();
    }

    fn sync(&mut self) {
        let now = chrono::Utc::now();
        self.notices.retain(|msg| {
            let elapsed = now.signed_duration_since(msg.created_at);
            elapsed.num_milliseconds()
                < msg
                    .value
                    .duration()
                    .unwrap_or(self.display_duration)
                    .as_millis() as i64
        });
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        self.sync();
        if self.notices.is_empty() {
            return;
        }

        let max_width = area.width as usize - 2;
        let max_height = area.height as usize - 2;

        let items = build_list_items(&self.notices, max_width, max_height);
        let list = List::new(items);
        f.render_widget(list, area);
    }
}

impl Default for Notice {
    fn default() -> Self {
        Self {
            notices: vec![],
            display_duration: Duration::from_secs(3),
        }
    }
}

fn build_list_items<'a>(
    notices: &[MessageWrapper],
    max_width: usize,
    max_height: usize,
) -> Vec<ListItem<'a>> {
    let mut items = vec![];
    let mut current_height = 0;

    for item in notices {
        let lines = build_bubble(
            item.value.message(),
            max_width,
            item.value.message_type().color(),
        );

        current_height += lines.len();
        if current_height > max_height {
            break;
        }

        let item = ListItem::new(lines).style(Style::default());
        items.push(item);
    }
    items
}

fn build_bubble<'a>(message: &str, max_width: usize, border_color: Color) -> Vec<Line<'a>> {
    // build lines from message based on max_width
    let mut lines = vec![];

    let mut line = String::new();
    for word in message.replace('\n', " ").split(' ') {
        if line.width() + word.width() > max_width - 2 {
            lines.push(line.trim().to_string());
            line = String::new();
        }
        line.push_str(word);
        line.push(' ');
    }

    if !line.is_empty() {
        lines.push(line.trim().to_string());
    }

    wrap_bubble(lines, max_width, border_color)
}

fn wrap_bubble<'a>(lines: Vec<String>, max_width: usize, border_color: Color) -> Vec<Line<'a>> {
    let top_bar = highlight_line(
        format!("╭{}╮", ["─"].repeat(max_width).join("")),
        border_color,
    );
    let bottom_bar = highlight_line(
        format!("╰{}╯", ["─"].repeat(max_width).join("")),
        border_color,
    );

    let mut wrapped_lines = vec![top_bar];
    for line in lines {
        let fill = helpers::repeat_from_substactions(" ", vec![max_width - 2, line.width()]);
        wrapped_lines.push(highlight_line(format!("│ {line}{fill} │"), border_color));
    }

    wrapped_lines.push(bottom_bar);
    wrapped_lines
}

fn highlight_span<'a>(text: String, color: Color) -> Span<'a> {
    Span::styled(
        text,
        Style {
            fg: Some(color),
            ..Default::default()
        },
    )
}

fn highlight_line<'a>(text: String, color: Color) -> Line<'a> {
    Line::from(highlight_span(text, color))
}
