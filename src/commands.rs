use anyhow::Result;
use clap::{arg, command, Parser, Subcommand};
use crossterm::{cursor, execute, style::Color, terminal::ClearType};
use tabled::builder::Builder;

use crate::{terminal::eprintln_colored, terminal::println, util::base_table_settings, Context};

// TODO: https://docs.rs/clap/latest/clap/_cookbook/repl/index.html

#[derive(Debug, Parser)]
#[command()]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Clear the terminal
    Clear,
    /// Exit the program
    #[command(aliases = ["quit", "q"])]
    Exit,
    /// Get or modify variables
    // TODO: get, set, and remove as subcommands (https://docs.rs/clap/latest/clap/_derive/_cookbook/git/index.html)
    Var {
        /// The name of the variable
        name: Option<String>,
        /// The value to set the variable to
        #[arg(requires = "name")]
        value: Option<String>,
    },
}

impl Context {
    pub(crate) async fn handle_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::Clear => execute!(
                std::io::stdout(),
                crossterm::terminal::Clear(ClearType::All),
                cursor::MoveTo(0, 0)
            )?,
            Command::Var { name, value } => self.handle_variable_command(name, value).await?,
            Command::Exit => {}
        };

        Ok(())
    }

    async fn handle_variable_command(
        &mut self,
        name: Option<String>,
        value: Option<String>,
    ) -> Result<()> {
        if let Some(name) = name {
            if let Some(value) = value {
                self.variables.insert(name.clone(), value);
            }
            println(
                self.variables
                    .get(&name)
                    .unwrap_or(&format!("Variable `{name}` is currently unset.")),
            )?;
        } else {
            if self.variables.is_empty() {
                eprintln_colored("There are currently no variables.", Color::Red)?;
            } else {
                let mut builder = Builder::default();
                builder.set_header(["Name", "Value"]);
                for (name, value) in &self.variables {
                    builder.push_record([name, value]);
                }
                let table = builder.build().with(base_table_settings()).to_string();
                println(table)?;
            }
        }
        Ok(())
    }
}
