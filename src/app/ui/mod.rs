pub mod bubble;
pub mod bubble_list;
pub mod edit;
pub mod help;
pub mod history;
pub mod input_box;
pub mod loading;
pub mod models;
pub mod notice;
pub mod question;
pub mod scroll;
pub mod selections;
pub mod syntaxes;
pub mod textarea;
pub mod utils;

pub use bubble::Bubble;
pub use bubble_list::BubbleList;

pub use edit::EditScreen;
pub use help::HelpScreen;
pub use history::HistoryScreen;
pub use loading::Loading;
pub use models::ModelsScreen;
pub use notice::Notice;
use ratatui::{
    style::{Color, Modifier, Style},
    widgets::Block,
};
pub use scroll::Scroll;
pub use textarea::TextArea;

pub trait Dim {
    fn dim_bg(&mut self);
}

impl Dim for ratatui::Frame<'_> {
    fn dim_bg(&mut self) {
        self.render_widget(
            Block::default().style(
                Style::default()
                    .bg(Color::Rgb(0, 0, 0))
                    .add_modifier(Modifier::DIM),
            ),
            self.area(),
        );
    }
}
