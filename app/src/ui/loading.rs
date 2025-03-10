use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

#[derive(Default)]
pub struct Loading(String);

impl Loading {
    pub fn new(text: &str) -> Self {
        Self(text.to_string())
    }

    fn text(&self) -> &str {
        if self.0.is_empty() {
            "Loading..."
        } else {
            &self.0
        }
    }

    pub fn render(&self, frame: &mut Frame, rect: Rect) {
        frame.render_widget(
            Paragraph::new(self.text())
                .style(Style {
                    add_modifier: Modifier::ITALIC,
                    ..Default::default()
                })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .padding(Padding::new(1, 1, 0, 0)),
                ),
            rect,
        );
    }
}
