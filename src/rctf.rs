use crate::{
    commands::Command,
    session::{SessionSelection, SessionType},
    ssh::SshSettings,
    terminal::{eprintln_colored, println},
    util::base_table,
    Context,
};
use anyhow::Result;
use clap::{arg, command, value_parser, Parser, Subcommand};
use crossterm::style::Color;

#[derive(Debug, Parser)]
#[command()]
struct Rctf {
    #[command(subcommand)]
    command: RctfCommand,
}

#[derive(Debug, Subcommand)]
enum RctfCommand {
    /// SSH into a remote host
    Ssh {
        /// User to connect as
        username: String,
        /// Destination hostname or IP to connect to
        hostname: String,
        /// Password to authenticate with
        #[arg(long)]
        password: Option<String>,
        /// Port to use
        #[arg(short, long, default_value_t = 22, value_parser = value_parser!(u16).range(1..))]
        port: u16,
    },
    /// List or use sessions
    #[group(required = false)]
    // TODO: https://docs.rs/clap/latest/clap/_derive/_cookbook/git/index.html
    Session {
        /// Name of the session to resume
        name: Option<String>,
        /// Index of the session to resume
        index: Option<usize>,
    },

    #[command(flatten)]
    Command(Command),
}

impl Context {
    pub(crate) async fn start_read_loop(&mut self) -> Result<()> {
        const PROMPT: &str = env!("CARGO_PKG_NAME");

        loop {
            let mut new_history = self.rctf_history.clone();
            let res = self.get_next_command(PROMPT, &mut new_history).await;
            self.rctf_history = new_history;

            let cmd: Rctf = match res {
                Ok(cmd) => cmd,
                Err(e) => {
                    eprintln_colored(e, Color::Red)?;
                    continue;
                }
            };

            match cmd.command {
                RctfCommand::Ssh {
                    username,
                    hostname,
                    password,
                    port,
                } => {
                    if let Err(e) = self
                        .start_session(SessionType::Ssh(SshSettings {
                            hostname,
                            port,
                            username,
                            password: password.unwrap_or(String::new()),
                        }))
                        .await
                    {
                        eprintln_colored(e, Color::Red)?;
                    }
                }
                RctfCommand::Session { name, index } => {
                    if let Err(e) = self.handle_session_command(name, index).await {
                        eprintln_colored(e, Color::Red)?;
                    }
                }
                RctfCommand::Command(Command::Exit) => break,
                RctfCommand::Command(command) => {
                    if let Err(e) = self.handle_command(command).await {
                        eprintln_colored(e, Color::Red)?;
                    }
                }
            };
        }

        Ok(())
    }

    async fn handle_session_command(
        &mut self,
        name: Option<String>,
        index: Option<usize>,
    ) -> Result<()> {
        if let Some(name) = name {
            self.resume_session(SessionSelection::Name(name)).await?;
        } else if let Some(index) = index {
            self.resume_session(SessionSelection::Index(index)).await?;
        } else {
            if self.sessions.is_empty() {
                eprintln_colored("There are currently no sessions.", Color::Red)?;
            } else {
                println(base_table(
                    self.sessions.iter().flatten().map(|item| item.as_ref()),
                ))?;
            }
        }

        Ok(())
    }
}
