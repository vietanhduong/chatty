use once_cell::sync::Lazy;
use ratatui::style::Color;
use std::{env, fs};
use syntect::parsing::{SyntaxReference, SyntaxSet};

pub static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(Syntaxes::load);

pub struct Syntaxes;

impl Syntaxes {
    fn load() -> SyntaxSet {
        if let Ok(path) = env::var("SYNTAX_BIN") {
            let bin = fs::read_to_string(path).unwrap();
            return bincode::deserialize_from(&bin.as_bytes()[..]).unwrap();
        }
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
        match synntect_color {
            syntect::highlighting::Color { r, g, b, .. } => Some(Color::Rgb(r, g, b)),
        }
    }
}
