pub mod app;
pub mod app_state;
pub mod services;
pub mod ui;

use std::io;

pub use app::App;

use crossterm::{
    cursor,
    event::{DisableBracketedPaste, DisableMouseCapture},
    terminal::{LeaveAlternateScreen, disable_raw_mode, is_raw_mode_enabled},
};

pub fn destruct_terminal_for_panic() {
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
