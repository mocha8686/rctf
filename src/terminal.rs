#![allow(dead_code)]

use std::{fmt::Display, io::Write};

use anyhow::Result;
use crossterm::{
    event::{self, KeyboardEnhancementFlags},
    execute, queue,
    style::{self, Color},
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

pub fn println<T: Display>(item: T) -> Result<()> {
    println_helper(&mut std::io::stdout(), item, None)
}

pub fn println_colored<T: Display>(item: T, color: Color) -> Result<()> {
    println_helper(&mut std::io::stdout(), item, Some(color))
}

pub fn eprintln<T: Display>(item: T) -> Result<()> {
    println_helper(&mut std::io::stderr(), item, None)
}

pub fn eprintln_colored<T: Display>(item: T, color: Color) -> Result<()> {
    println_helper(&mut std::io::stderr(), item, Some(color))
}

fn println_helper<T, W>(writer: &mut W, item: T, color: Option<Color>) -> Result<()>
where
    W: Write,
    T: Display,
{
    disable_raw_mode()?;
    if let Some(color) = color {
        execute!(
            writer,
            style::SetForegroundColor(color),
            style::Print(item),
            style::ResetColor,
            style::Print("\n"),
        )?;
    } else {
        execute!(writer, style::Print(item), style::Print("\n"))?;
    }
    enable_raw_mode()?;

    Ok(())
}
