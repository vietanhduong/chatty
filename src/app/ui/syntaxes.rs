use once_cell::sync::Lazy;
use ratatui::style::Color;
use syntect::parsing::{SyntaxReference, SyntaxSet};

pub static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(Syntaxes::load);

pub struct Syntaxes;

impl Syntaxes {
    fn load() -> SyntaxSet {
        SyntaxSet::load_defaults_newlines()
    }

    pub fn get(name: &str) -> &SyntaxReference {
        if let Some(syntax) = SYNTAX_SET.find_syntax_by_extension(name) {
            return syntax;
        }

        if let Some(syntax) = SYNTAX_SET.find_syntax_by_name(name) {
            return syntax;
        }

        if let Some(syntax) = SYNTAX_SET.find_syntax_by_token(name) {
            return syntax;
        }

        SYNTAX_SET.find_syntax_plain_text()
    }

    pub fn list() -> Vec<String> {
        let mut list = SYNTAX_SET
            .syntaxes()
            .iter()
            .map(|s| s.name.clone())
            .collect::<Vec<String>>();
        list.sort();
        list
    }

    pub fn translate_colour(synntect_color: syntect::highlighting::Color) -> Option<Color> {
        let syntect::highlighting::Color { r, g, b, .. } = synntect_color;
        Some(Color::Rgb(r, g, b))
    }
}
