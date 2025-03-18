use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Text},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

#[derive(Default)]
pub struct Loading<'a>(Line<'a>);

impl<'a> Loading<'a> {
    pub fn new(text: impl Into<Line<'a>>) -> Loading<'a> {
        Loading(text.into())
    }

    fn value(&self) -> &Line {
        &self.0
    }

    pub fn render(&self, frame: &mut Frame, rect: Rect) {
        frame.render_widget(
            Paragraph::new(Text::from(vec![self.value().clone()]))
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
