use ratatui::{Frame, layout::Rect, style::Stylize, text::Line};

pub fn render_instruction(frame: &mut Frame, area: Rect) {
    let instructions = vec!["<Ctrl+h>: [H]istory", "<Ctrl+q>: [Q]uit", "<Ctrl+?>: Help"];

    let line = Line::from(instructions.join(" | ")).blue();
    frame.render_widget(line, area);
}
