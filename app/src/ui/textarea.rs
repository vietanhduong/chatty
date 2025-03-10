use ratatui::{
    layout::Alignment,
    widgets::{Block, BorderType, Borders, Padding},
};

pub struct TextArea {
    title: String,
    placeholder: String,
}

impl TextArea {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn build<'a>(&self) -> tui_textarea::TextArea<'a> {
        let mut textarea = tui_textarea::TextArea::default();
        textarea.set_block(
            Block::default()
                .title(self.title.clone())
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title_alignment(Alignment::Left)
                .padding(Padding::new(1, 1, 0, 0)),
        );
        textarea.set_placeholder_text(self.placeholder.clone());
        textarea
    }
}

impl Default for TextArea {
    fn default() -> Self {
        Self {
            title: " Input ".to_string(),
            placeholder: "Type your message here...".to_string(),
        }
    }
}
