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
pub mod selection;
pub mod syntaxes;
pub mod textarea;
pub mod utils;

pub use bubble::Bubble;
pub use bubble_list::BubbleList;
pub use selection::Selection;

pub use edit::EditScreen;
pub use help::HelpScreen;
pub use history::HistoryScreen;
pub use loading::Loading;
pub use models::ModelsScreen;
pub use notice::Notice;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
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

pub trait ContainModifier {
    fn contains(&self, modifier: Modifier) -> bool;
    fn add_contains(&self, modifier: Modifier) -> bool;
    fn sub_contains(&self, modifier: Modifier) -> bool;
}

impl ContainModifier for Style {
    fn contains(&self, modifier: Modifier) -> bool {
        self.add_modifier.contains(modifier) || self.sub_modifier.contains(modifier)
    }

    fn add_contains(&self, modifier: Modifier) -> bool {
        self.add_modifier.contains(modifier)
    }

    fn sub_contains(&self, modifier: Modifier) -> bool {
        self.sub_modifier.contains(modifier)
    }
}

pub trait Content {
    fn content(&self) -> String;
}

impl Content for Vec<Span<'_>> {
    fn content(&self) -> String {
        self.iter()
            .map(|s| s.content.to_string())
            .collect::<Vec<String>>()
            .join("")
    }
}

impl Content for &[Span<'_>] {
    fn content(&self) -> String {
        self.iter()
            .map(|s| s.content.to_string())
            .collect::<Vec<String>>()
            .join("")
    }
}

impl Content for Line<'_> {
    fn content(&self) -> String {
        self.spans.content()
    }
}

pub trait Selectable {
    fn is_selectable(&self) -> bool;
    fn selectable(self) -> Self;
    fn unselectable(self) -> Self;
    fn highlighted(self) -> Self;
}

impl Selectable for Span<'_> {
    fn is_selectable(&self) -> bool {
        !self.style.contains(Modifier::HIDDEN)
    }
    fn selectable(mut self) -> Self {
        self.style.add_modifier.remove(Modifier::HIDDEN);
        self
    }
    fn unselectable(mut self) -> Self {
        self.style.add_modifier.insert(Modifier::HIDDEN);
        self
    }

    fn highlighted(mut self) -> Self {
        self.style = self.style.fg(Color::Black).bg(Color::Cyan);
        self
    }
}

impl Selectable for Line<'_> {
    fn is_selectable(&self) -> bool {
        !self.style.contains(Modifier::HIDDEN)
    }
    fn selectable(mut self) -> Self {
        self.style.add_modifier.remove(Modifier::HIDDEN);
        self
    }
    fn unselectable(mut self) -> Self {
        self.style.add_modifier.insert(Modifier::HIDDEN);
        self
    }
    fn highlighted(mut self) -> Self {
        self.style = self.style.fg(Color::Black).bg(Color::Cyan);
        self
    }
}
