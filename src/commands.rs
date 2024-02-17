use anyhow::Result;
use clap::{arg, command, Parser, Subcommand};
use crossterm::{cursor, execute, style::Color, terminal::ClearType};
use tabled::builder::Builder;

use crate::{terminal::eprintln_colored, terminal::println, util::table_settings, Context};

// TODO: https://docs.rs/clap/latest/clap/_cookbook/repl/index.html

#[derive(Debug, Parser)]
#[command(multicall = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
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

impl<'a> Context<'a> {
    pub async fn handle_command(&mut self, command: Commands) -> Result<()> {
        match command {
            Commands::Clear => execute!(
                std::io::stdout(),
                crossterm::terminal::Clear(ClearType::All),
                cursor::MoveTo(0, 0)
            )?,
            Commands::Var { name, value } => self.variable(name, value).await?,
            Commands::Exit => {}
        };

        Ok(())
    }

    async fn variable(&mut self, name: Option<String>, value: Option<String>) -> Result<()> {
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
                let table = Table::builder(&self.variables);
                // TODO: test
                // builder.set_header(["name", "value"]);

                let table = table.build().with(table_settings()).to_string();
                println(table)?;
            }
        }
        Ok(())
    }
}
