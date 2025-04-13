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
    event::{
        DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode, is_raw_mode_enabled,
    },
};
use eyre::{Context, Result};

pub fn destruct_terminal() {
    if let Ok(enabled) = is_raw_mode_enabled() {
        if enabled {
            let _ = disable_raw_mode();
            let _ = crossterm::execute!(
                io::stdout(),
                DisableMouseCapture,
                PopKeyboardEnhancementFlags,
                DisableBracketedPaste,
                DisableFocusChange,
                LeaveAlternateScreen,
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
        EnableFocusChange,
        Clear(ClearType::All),
        EnableMouseCapture,
        EnableBracketedPaste,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
        )
    )?;
    Ok(())
}
