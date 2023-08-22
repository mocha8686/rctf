use std::io::Write;

use anyhow::Result;
use crossterm::{
    event::{self, KeyboardEnhancementFlags},
    execute, queue,
    terminal::{disable_raw_mode, enable_raw_mode},
};

pub fn setup(supports_keyboard_enhancement: bool) -> Result<()> {
    enable_raw_mode()?;

    let mut stdout = std::io::stdout();
    if supports_keyboard_enhancement {
        queue!(
            stdout,
            event::PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )?;
    }
    execute!(
        stdout,
        event::EnableBracketedPaste,
        event::EnableFocusChange,
        event::EnableMouseCapture,
    )?;

    Ok(())
}

pub fn teardown(supports_keyboard_enhancement: bool) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.flush()?;

    disable_raw_mode()?;

    if supports_keyboard_enhancement {
        queue!(stdout, event::PopKeyboardEnhancementFlags)?;
    }

    execute!(
        stdout,
        event::DisableBracketedPaste,
        event::DisableFocusChange,
        event::DisableMouseCapture,
    )?;

    Ok(())
}
