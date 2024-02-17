use crate::{
    commands::Commands,
    session::SessionSelection,
    ssh::{SshSession, SshSettings},
    terminal::{eprintln_colored, println},
    util::table_settings,
    Context,
};
use anyhow::Result;
use clap::{arg, command, value_parser, Parser, Subcommand};
use crossterm::style::Color;
use tabled::Table;

// TODO: https://docs.rs/clap/latest/clap/_cookbook/repl_derive/index.html
#[derive(Debug, Parser)]
#[command(multicall = true)]
struct Rctf {
    #[command(subcommand)]
    command: RctfCommands,
}

#[derive(Debug, Subcommand)]
enum RctfCommands {
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
    Session {
        /// Name of the session to resume
        name: Option<String>,
        /// Index of the session to resume
        index: Option<usize>,
    },

    #[command(flatten)]
    Command(Commands),
}

impl<'a> Context<'a> {
    pub async fn start_read_loop(&mut self) -> Result<()> {
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
                RctfCommands::Ssh {
                    username,
                    hostname,
                    password,
                    port,
                } => {
                    let settings = SshSettings {
                        hostname,
                        port,
                        username,
                        password: password.unwrap_or(String::new()),
                    };
                    let ssh = SshSession::new(settings);
                    if let Err(e) = self.start_session(ssh).await {
                        eprintln_colored(e, Color::Red)?;
                    }
                }
                RctfCommands::Session { name, index } => {
                    if let Err(e) = self.session(name, index).await {
                        eprintln_colored(e, Color::Red)?;
                    }
                }
                RctfCommands::Command(Commands::Exit) => break,
                RctfCommands::Command(command) => {
                    if let Err(e) = self.handle_command(command).await {
                        eprintln_colored(e, Color::Red)?;
                    }
                }
            };
        }

        Ok(())
    }

    async fn session(&mut self, name: Option<String>, index: Option<usize>) -> Result<()> {
        if let Some(name) = name {
            self.resume_session(SessionSelection::Name(name)).await?;
        } else if let Some(index) = index {
            self.resume_session(SessionSelection::Index(index)).await?;
        } else {
            if self.sessions.is_empty() {
                println("There are currently no sessions.")?;
            } else {
                let mut table = Table::builder(
                    self.sessions
                        .iter()
                        .flatten()
                        .enumerate()
                        .map(|(i, s)| (i, s.type_name(), s.name().unwrap_or(""))),
                );
                table.set_header(["index", "name", "type"]);

                let table = table.build().with(table_settings()).to_string();
                println(table)?;
            }
        }

        Ok(())
    }
}
