#[allow(clippy::module_inception)]
pub mod app;
pub mod app_state;
pub mod initializer;
pub mod services;
pub mod ui;

use std::io;

pub use app::App;
pub use initializer::Initializer;

use crossterm::{
    cursor,
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        is_raw_mode_enabled,
    },
};
use eyre::{Context, Result};

pub fn destruct_terminal() {
    if let Ok(enabled) = is_raw_mode_enabled() {
        if enabled {
            let _ = disable_raw_mode();
            let _ = crossterm::execute!(
                io::stdout(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                DisableBracketedPaste
            );
            let _ = crossterm::execute!(io::stdout(), cursor::Show);
        }
    }
}

pub fn init_terminal() -> Result<()> {
    let mut stdout = io::stdout();
    enable_raw_mode().wrap_err("enabling raw mode")?;
    crossterm::execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    Ok(())
}
