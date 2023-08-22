use std::io::{self, Write};

use anyhow::Result;
use constants::EOT;
use crossterm::{
    cursor,
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    style::{self, Color},
    terminal::ClearType,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use terminal::{setup_terminal, teardown_terminal};

pub mod constants;
pub mod ssh;
pub mod terminal;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    ip: String,
    port: u16,
    username: String,
    password: String,
}

#[derive(Clone, Debug)]
pub struct Context {
    ssh_settings: Settings,
    supports_keyboard_enhancement: bool,
}

impl Context {
    pub fn new(ssh_settings: Settings) -> Result<Self> {
        Ok(Self {
            ssh_settings,
            supports_keyboard_enhancement: crossterm::terminal::supports_keyboard_enhancement()?,
        })
    }

    pub async fn start(mut self) -> Result<()> {
        setup_terminal(self.supports_keyboard_enhancement)?;
        let res = self.start_read_loop().await;
        teardown_terminal(self.supports_keyboard_enhancement)?;
        res
    }

    async fn start_read_loop(&mut self) -> Result<()> {
        let mut stdout = io::stdout();
        let mut reader = EventStream::new();

        'outer: loop {
            execute!(
                stdout,
                style::SetForegroundColor(Color::Blue),
                style::Print("rctf"),
                style::ResetColor,
                style::Print("> "),
            )?;

            let mut cmd = String::new();

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
                    let data = match (code, modifiers) {
                        (KeyCode::Esc, _) => {
                            write!(stdout, "\r\n")?;
                            break 'outer;
                        }
                        (KeyCode::Enter, _) => {
                            write!(stdout, "\r\n")?;
                            break;
                        }
                        (KeyCode::Backspace, _) => {
                            if cmd.is_empty() {
                                continue;
                            }

                            execute!(
                                stdout,
                                cursor::MoveLeft(1),
                                crossterm::terminal::Clear(ClearType::UntilNewLine),
                            )?;
                            cmd.pop();

                            continue;
                        }
                        // (KeyCode::Up, _) => todo!(),
                        // (KeyCode::Down, _) => todo!(),
                        // (KeyCode::Right, _) => todo!(),
                        // (KeyCode::Left, _) => todo!(),
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            write!(stdout, "^C\r\n")?;
                            continue 'outer;
                        }
                        (KeyCode::Char('d'), KeyModifiers::CONTROL) => EOT as char,
                        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => c,
                        _ => continue,
                    };
                    write!(stdout, "{}", data)?;
                    stdout.flush()?;
                    cmd.push(data);
                }
            }

            match &*cmd.trim() {
                "exit" => break 'outer,
                "clear" => execute!(stdout, crossterm::terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?,
                "ssh" => {
                    self.start_ssh().await?;
                }
                _ => continue,
            }
        }

        Ok(())
    }
}
