use ratatui::style::Stylize;
use ratatui_macros::span;

use super::*;

#[test]
fn test_get_selected_columns_in_row() {
    let mut sel = Selection::default();
    sel.set_start(1, 6);
    sel.set_end(1, 2);

    assert_eq!(sel.get_selected_columns_in_row(1, 38), Some((2, 6)));

    sel.clear();
    sel.set_start(1, 2);
    sel.set_end(1, 6);

    assert_eq!(sel.get_selected_columns_in_row(1, 38), Some((2, 6)));
}

#[test]
fn test_format_line() {
    let line = Line::from(vec![
        span!("  ").unselectable(),
        span!("Hello Sir, How can I help you to day?"),
        span!("   ").unselectable(),
    ]);

    let mut sel = Selection::default();
    sel.set_start(1, 6);
    sel.set_end(1, 2);

    let formatted_line = sel.format_line(line, 1);
    let expected_line = Line::from(vec![
        span!("  ").unselectable(),
        span!("Hello").highlighted(),
        span!(" Sir, How can I help you to day?"),
        span!("   ").unselectable(),
    ]);
    assert_eq!(formatted_line, expected_line);

    let line = Line::from(vec![
        span!("  ").unselectable(),
        span!("Hello "),
        span!("Sir, How can").bold(),
        span!("I help you to day?"),
        span!("   ").unselectable(),
    ]);
    let mut sel = Selection::default();
    sel.set_start(1, 4);
    sel.set_end(1, 13);
    let formatted_line = sel.format_line(line, 1);
    let expected_line = Line::from(vec![
        span!("  ").unselectable(),
        span!("He"),
        span!("llo ").highlighted(),
        span!("Sir, H").highlighted(),
        span!("ow can").bold(),
        span!("I help you to day?"),
        span!("   ").unselectable(),
    ]);
    assert_eq!(formatted_line, expected_line);
}
