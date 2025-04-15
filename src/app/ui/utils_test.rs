use ratatui::style::{Modifier, Stylize};
use ratatui_macros::span;

use crate::app::ui::Content;

use super::*;

#[test]
fn test_split_to_lines() {
    let text = "This is a test string that is too long to fit in a single line.";
    let max_width = 20;
    let lines = split_to_lines(text, max_width);

    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].content(), "This is a test ");
    assert_eq!(lines[1].content(), "string that is too ");
    assert_eq!(lines[2].content(), "long to fit in a ");
    assert_eq!(lines[3].content(), "single line.");
}

#[test]
fn test_split_to_lines_contains_styled_span() {
    let text = vec![
        span!("This "),
        span!("is styled span").bold(),
        span!(" can"),
        span!(" be"),
        span!(" split"),
    ];

    let max_width = 7;
    let lines = split_to_lines(text, max_width);
    let bold = Style::default().add_modifier(Modifier::BOLD);

    assert_eq!(lines.len(), 6);

    let wspan = wrapper_span();

    assert_eq!(lines[0].content(), "This ");
    assert_eq!(lines[1].content(), "is ");
    check_span_style(&lines[1], &[bold.clone(), bold.clone(), wspan.style]);
    assert_eq!(lines[2].content(), "styled");
    check_span_style(&lines[2], &[bold.clone(), wspan.style]);
    assert_eq!(lines[3].content(), " span ");
    check_span_style(
        &lines[3],
        &[bold.clone(), bold, Style::default(), wspan.style],
    );
    assert_eq!(lines[4].content(), "can be");
    assert_eq!(lines[5].content(), " split");
}

#[test]
fn test_is_wrapper_span() {
    let span = wrapper_span();
    assert!(is_wrapper_span(&span));
    let span = Span::styled("a", Style::default());
    assert!(!is_wrapper_span(&span));

    let mut cfg = config::Configuration::default();
    cfg.general.show_wrapped_indicator = Some(false);
    config::init(cfg).expect("Failed to initialize config");

    let span = wrapper_span();
    assert_eq!(span.content, "");
    assert!(is_wrapper_span(&span));
}

fn check_span_style(line: &Line, styles: &[Style]) {
    assert_eq!(line.spans.len(), styles.len());
    for (i, span) in line.spans.iter().enumerate() {
        assert_eq!(span.style, styles[i]);
    }
}
