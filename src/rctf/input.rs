use std::io::{self, Write};

use crate::{constants::MAX_HISTORY_SIZE, Context};
use anyhow::Result;
use crossterm::{
    cursor,
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, style,
    terminal::{self, ClearType},
};
use futures::StreamExt;

impl Context {
    #[allow(clippy::too_many_lines)]
    pub(super) async fn get_next_command(&mut self) -> Result<Option<String>> {
        const PROMPT_LENGTH: usize = 6;

        let mut history = self.rctf_history.clone();
        let history_len = history.len();
        let mut stdout = io::stdout();
        let mut reader = EventStream::new();
        let mut current_cmd = String::new();
        let mut cmd = &mut current_cmd;
        let mut column = 0usize;
        let mut history_index = self.rctf_history.len();

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
                        cmd = history.get_mut(history_index).unwrap();
                        column = cmd.len();
                        execute!(
                            stdout,
                            cursor::MoveToColumn(PROMPT_LENGTH as u16),
                            terminal::Clear(ClearType::UntilNewLine),
                            style::Print(&cmd),
                        )?;
                    }
                    (KeyCode::Down, _) => {
                        if history_index == history_len {
                            continue;
                        }

                        history_index += 1;
                        if history_index == self.rctf_history.len() {
                            cmd = &mut current_cmd;
                        } else {
                            cmd = history.get_mut(history_index).unwrap();
                        }
                        column = cmd.len();
                        execute!(
                            stdout,
                            cursor::MoveToColumn(PROMPT_LENGTH as u16),
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

        self.rctf_history.push_back(cmd.clone());
        while self.rctf_history.len() > MAX_HISTORY_SIZE {
            self.rctf_history.pop_front();
        }
        Ok(Some((*cmd).to_string()))
    }
}
