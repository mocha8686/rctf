use std::io::{self, Write};

use crate::{constants::MAX_HISTORY_SIZE, ssh::SshSettings, Context};
use anyhow::Result;
use clap::{arg, command, value_parser, Parser, Subcommand};
use crossterm::{
    cursor,
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    style::{self, Color},
    terminal::{disable_raw_mode, enable_raw_mode, ClearType},
};
use futures::StreamExt;

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
        /// User to connect as
        username: String,
        /// Destination hostname or IP to connect to
        hostname: String,
        /// Password to authenticate with
        password: Option<String>,
        /// Port to use
        #[arg(short, long, default_value_t = 22, value_parser = value_parser!(u16).range(1..))]
        port: u16,
    },
}

impl Context {
    pub(crate) async fn start_read_loop(&mut self) -> Result<()> {
        let mut stdout = io::stdout();
        let mut stderr = io::stderr();

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

            let cmd = match shlex::split(&cmd) {
                None => {
                    execute!(
                        stderr,
                        style::SetForegroundColor(Color::Red),
                        style::Print("Invalid quoting.\r\n"),
                        style::ResetColor,
                    )?;
                    continue;
                }
                Some(cmd) if cmd.is_empty() => {
                    continue;
                }
                Some(cmd) => cmd,
            };

            let cmd = match Rctf::try_parse_from(["rctf".into()].into_iter().chain(cmd.into_iter()))
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
                        stderr,
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
                Command::Ssh {
                    username,
                    hostname,
                    password,
                    port,
                } => {
                    if let Err(e) = self
                        .start_ssh(SshSettings {
                            hostname,
                            port,
                            username,
                            password: password.unwrap_or("".into()),
                        })
                        .await
                    {
                        execute!(
                            stderr,
                            style::SetForegroundColor(Color::Red),
                            style::Print(format!("{}\r\n", e)),
                            style::ResetColor,
                        )?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn get_next_command(&mut self) -> Result<Option<String>> {
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
                            crossterm::terminal::Clear(ClearType::UntilNewLine),
                            cursor::SavePosition,
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
                            cursor::MoveToColumn((column + PROMPT_LENGTH) as u16),
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
                            cursor::MoveToColumn((column + PROMPT_LENGTH) as u16),
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
                            crossterm::terminal::Clear(ClearType::UntilNewLine),
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
        Ok(Some(cmd.to_string()))
    }
}
