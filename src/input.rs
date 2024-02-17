use std::io::{self, Write};

use crate::{terminal::println, CommandHistory, Context};
use anyhow::{bail, Result};
use clap::Parser;
use crossterm::{
    cursor,
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    style::{self, Color},
    terminal::{self, ClearType},
};
use futures::StreamExt;
use lazy_static::lazy_static;
use pcre2::bytes::Regex;

pub const MAX_HISTORY_SIZE: usize = 100;

lazy_static! {
    // https://regex101.com/r/BsuPom/1
    static ref VARIABLE_REGEX: Regex =
        Regex::new(r"(?<!(?<!\\)\\)(?:#([A-Za-z]\w*)|#\{([A-Za-z]\w*)\})").unwrap();

    // https://regex101.com/r/oTfnpy/1
    static ref BYTES_REGEX: Regex = Regex::new(r"(?<!\\)\\(x[0-9A-Fa-f]{2}|u[0-9A-Fa-f]{4})").unwrap();
}

impl Context {
    pub(crate) async fn get_next_command<P>(
        &self,
        prompt: &str,
        history: &mut CommandHistory,
    ) -> Result<P>
    where
        P: Parser,
    {
        loop {
            let Some(next_line) = get_next_line(prompt, history).await? else {
                continue;
            };

            let next_line = self.expand(&next_line)?;

            let args = match shlex::split(&next_line) {
                None => {
                    execute!(
                        std::io::stderr(),
                        style::SetForegroundColor(Color::Red),
                        style::Print("Invalid quoting.\r\n"),
                        style::ResetColor,
                    )?;
                    continue;
                }
                Some(args) if args.is_empty() => {
                    continue;
                }
                Some(args) => args,
            };

            let cmd = match P::try_parse_from([prompt.into()].into_iter().chain(args)) {
                Ok(cmd) => cmd,
                Err(e) if e.kind() == clap::error::ErrorKind::DisplayHelp => {
                    println(e)?;
                    continue;
                }
                Err(e) => {
                    bail!(e);
                }
            };

            return Ok(cmd);
        }
    }

    pub(crate) fn expand(&self, input: &str) -> Result<String> {
        // special chars.
        // `\n`        new line
        // `\r`        carriage return
        // `\t`        horizontal tab
        let special_chars = input
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t");

        // variables
        // `\#`        hashtag
        let mut expanded_variables = String::new();
        let mut last_match = 0;
        for res in VARIABLE_REGEX.captures_iter(special_chars.as_bytes()) {
            let cap = res?;
            let capture_match = cap.get(0).unwrap();
            let variable_name =
                std::str::from_utf8(cap.get(1).unwrap_or_else(|| cap.get(2).unwrap()).as_bytes())?;
            let Some(value) = self.variables.get(variable_name) else {
                bail!(format!("Variable {variable_name} is not defined."));
            };

            expanded_variables.push_str(&special_chars[last_match..capture_match.start()]);
            expanded_variables.push_str(&self.expand(value)?);
            last_match = capture_match.end();
        }
        expanded_variables.push_str(&special_chars[last_match..]);

        // double backslash
        // `\\`        backslash
        Ok(expanded_variables.replace(r"\\", r"\"))
    }
}

// fn parse_byte_escape_string(input: &str) -> Result<Vec<u8>> {
//     TODO: ignore double backslashes
//
//     let data = &input[1..];
//     match input.chars().next().unwrap() {
//         'x' => {
//             let byte = u8::from_str_radix(data, 16)?;
//             Ok(vec![byte])
//         }
//         'u' => {
//             let codepoint = u8::from_str_radix(data, 16)?;
//             let codepoint = &[codepoint];
//
//             let codepoint_string = std::str::from_utf8(codepoint)?;
//             Ok(codepoint_string.bytes().collect_vec())
//         }
//         _ => bail!("Invalid byte escape"),
//     }
// }

fn print_prompt(prompt: &str) -> Result<()> {
    let mut stdout = io::stdout();

    execute!(
        stdout,
        style::SetForegroundColor(Color::Blue),
        style::Print(prompt),
        style::ResetColor,
        style::Print("> "),
    )?;

    Ok(())
}

async fn get_next_line(prompt: &str, history: &mut CommandHistory) -> Result<Option<String>> {
    let prompt_length = prompt.len() + 2;

    let mut stdout = io::stdout();
    let mut reader = EventStream::new();

    let mut history_clone = history.clone();
    let history_len = history.len();
    let mut current_cmd = String::new();
    let mut cmd = &mut current_cmd;
    let mut column = 0usize;
    let mut history_index = history.len();

    print_prompt(prompt)?;

    loop {
        let Some(event) = reader.next().await else {
            break;
        };

        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press | KeyEventKind::Repeat,
            ..
        }) = event?
        {
            match (code, modifiers) {
                (KeyCode::Esc, _) => {
                    write!(stdout, "\r\n")?;
                    return Ok(Some("exit".to_string()));
                }
                (KeyCode::Enter, _) => {
                    write!(stdout, "\r\n")?;
                    break;
                }
                (KeyCode::Backspace, _) => {
                    if column == 0 {
                        continue;
                    }

                    column -= 1;
                    cmd.remove(column);
                    execute!(
                        stdout,
                        cursor::MoveLeft(1),
                        terminal::Clear(ClearType::UntilNewLine),
                        cursor::SavePosition,
                        style::Print(&cmd[column..]),
                        cursor::RestorePosition,
                    )?;
                }
                (KeyCode::Delete, _) => {
                    if column == cmd.len() {
                        continue;
                    }

                    cmd.remove(column);
                    execute!(
                        stdout,
                        cursor::SavePosition,
                        terminal::Clear(ClearType::UntilNewLine),
                        style::Print(&cmd[column..]),
                        cursor::RestorePosition,
                    )?;
                }
                (KeyCode::Up, _) => {
                    if history_index == 0 {
                        continue;
                    }

                    if history_index == history_len {
                        current_cmd = cmd.clone();
                    }
                    history_index -= 1;
                    cmd = history_clone.get_mut(history_index).unwrap();
                    column = cmd.len();
                    execute!(
                        stdout,
                        cursor::MoveToColumn(prompt_length as u16),
                        terminal::Clear(ClearType::UntilNewLine),
                        style::Print(&cmd),
                    )?;
                }
                (KeyCode::Down, _) => {
                    if history_index == history_len {
                        continue;
                    }

                    history_index += 1;
                    if history_index == history.len() {
                        cmd = &mut current_cmd;
                    } else {
                        cmd = history_clone.get_mut(history_index).unwrap();
                    }
                    column = cmd.len();
                    execute!(
                        stdout,
                        cursor::MoveToColumn(prompt_length as u16),
                        terminal::Clear(ClearType::UntilNewLine),
                        style::Print(&cmd),
                    )?;
                }
                (KeyCode::Left, _) => {
                    if column == 0 {
                        continue;
                    }

                    column -= 1;
                    execute!(stdout, cursor::MoveLeft(1),)?;
                }
                (KeyCode::Right, _) => {
                    if column == cmd.len() {
                        continue;
                    }

                    column += 1;
                    execute!(stdout, cursor::MoveRight(1),)?;
                }
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                    write!(stdout, "^C\r\n")?;
                    return Ok(None);
                }
                (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    cmd.insert(column, c);
                    execute!(
                        stdout,
                        style::Print(c),
                        cursor::SavePosition,
                        terminal::Clear(ClearType::UntilNewLine),
                        style::Print(&cmd[column + 1..]),
                        cursor::RestorePosition,
                    )?;
                    column += 1;
                }
                _ => {}
            }
        }
    }

    history.push_back(cmd.clone());
    while history.len() > MAX_HISTORY_SIZE {
        history.pop_front();
    }
    Ok(Some((*cmd).to_string()))
}
