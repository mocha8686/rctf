use std::io::{self, Write};

use anyhow::Result;
use clap::{command, Parser, Subcommand};
use constants::EOT;
use crossterm::{
    cursor,
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    style::{self, Color},
    terminal::{disable_raw_mode, enable_raw_mode, ClearType},
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use terminal::{setup_terminal, teardown_terminal};

pub mod constants;
pub mod ssh;
pub mod terminal;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Rctf {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Clear the terminal
    Clear,
    /// Exit the program
    #[command(aliases = ["quit", "q"])]
    Exit,
    /// SSH into a remote host
    Ssh {
        /// The destination IP to connect to
        ip: String,
        /// The port to connect to on the remote host
        #[arg(short, long, default_value_t = 22)]
        port: u16,
    },
}

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

        loop {
            execute!(
                stdout,
                style::SetForegroundColor(Color::Blue),
                style::Print("rctf"),
                style::ResetColor,
                style::Print("> "),
            )?;

            let Some(cmd) = self.get_next_command().await? else {
                continue;
            };

            if cmd.split_whitespace().count() == 0 {
                continue;
            }

            let cmd = match Rctf::try_parse_from(["rctf"].into_iter().chain(cmd.split_whitespace()))
            {
                Ok(cmd) => cmd,
                Err(e) if e.kind() == clap::error::ErrorKind::DisplayHelp => {
                    disable_raw_mode()?;
                    write!(stdout, "{}", e)?;
                    enable_raw_mode()?;
                    continue;
                }
                Err(e) => {
                    disable_raw_mode()?;
                    execute!(
                        io::stderr(),
                        style::SetForegroundColor(Color::Red),
                        style::Print(e),
                        style::ResetColor,
                    )?;
                    enable_raw_mode()?;
                    continue;
                }
            };

            match cmd.command {
                Command::Clear => execute!(
                    stdout,
                    crossterm::terminal::Clear(ClearType::All),
                    cursor::MoveTo(0, 0)
                )?,
                Command::Exit => break,
                Command::Ssh { .. } => self.start_ssh().await?,
            }
        }

        Ok(())
    }

    async fn get_next_command(&self) -> Result<Option<String>> {
        let mut stdout = io::stdout();
        let mut reader = EventStream::new();
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
                        return Ok(Some("exit".to_string()));
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
                    (KeyCode::Up, _) => todo!(),
                    (KeyCode::Down, _) => todo!(),
                    (KeyCode::Right, _) => todo!(),
                    (KeyCode::Left, _) => todo!(),
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        write!(stdout, "^C\r\n")?;
                        return Ok(None);
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

        Ok(Some(cmd))
    }
}
