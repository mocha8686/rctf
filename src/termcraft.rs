use crate::{
    commands::Commands,
    terminal::{eprintln_colored, println},
    Context,
};
use anyhow::{bail, Result};
use clap::{command, Parser, Subcommand};
use crossterm::style::Color;

pub enum TermcraftResponse {
    Cmd(String),
    Background,
    Exit,
}

#[derive(Debug, Parser)]
#[command(multicall = true)]
struct Termcraft {
    #[command(subcommand)]
    command: TermcraftCommands,
}

// TODO: base64, hex, xor, etc.
#[derive(Debug, Subcommand)]
enum TermcraftCommands {
    /// Send current session to background
    #[command(alias = "background")]
    Bg,
    /// Get or change the session name
    Name {
        /// The name to change this session to
        name: Option<String>,
    },
    /// Terminal-style printf (man 1 printf)
    Printf {
        /// Format string
        ///
        /// Variables can be used like `#variable` or `#{variable}`.
        ///
        /// Escaped sequences include:
        /// `\\`        backslash
        /// `\n`        new line
        /// `\r`        carriage return
        /// `\t`        horizontal tab
        /// `\#`        hashtag
        // TODO:
        // `\xHH`      byte with hex value HH
        // `\uHHHH`    Unicode character with hex value HHHH
        format_string: String,
    },

    #[command(flatten)]
    Command(Commands),
}

impl<'a> Context<'a> {
    pub async fn start_termcraft(&mut self, session_index: usize) -> Result<TermcraftResponse> {
        const PROMPT: &str = "termcraft";

        if self.sessions.get(session_index).is_none() {
            bail!("Could not find session with index {session_index}.");
        };

        loop {
            let mut new_history = self.termcraft_history.clone();
            let res = self.get_next_command(PROMPT, &mut new_history).await;
            self.termcraft_history = new_history;

            let cmd: Termcraft = match res {
                Ok(cmd) => cmd,
                Err(e) => {
                    eprintln_colored(e, Color::Red)?;
                    continue;
                }
            };

            match cmd.command {
                TermcraftCommands::Bg => return Ok(TermcraftResponse::Background),
                TermcraftCommands::Name { name } => {
                    if let Some(name) = name {
                        *self.sessions.get_mut(session_index).unwrap().name_mut() = name.clone();
                        self.named_sessions.insert(name, session_index);
                    }
                    println(
                        self.sessions
                            .get(session_index)
                            .unwrap()
                            .name()
                            .unwrap_or("This session is currently unnamed."),
                    )?;
                }
                TermcraftCommands::Printf { format_string } => {
                    let cmd = match self.parse_line(&format_string) {
                        Ok(cmd) => cmd,
                        Err(e) => {
                            eprintln_colored(e, Color::Red)?;
                            continue;
                        }
                    };
                    return Ok(TermcraftResponse::Cmd(cmd));
                }
                TermcraftCommands::Command(Commands::Exit) => return Ok(TermcraftResponse::Exit),
                TermcraftCommands::Command(command) => self.handle_command(command).await?,
            }
        }
    }
}
