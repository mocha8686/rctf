use std::collections::{HashMap, VecDeque};

use anyhow::Result;
use session::SessionManager;

pub(crate) mod commands;
pub mod files;
pub(crate) mod input;
pub mod rctf;
mod session;
mod ssh;
mod termcraft;
pub(crate) mod terminal;
pub(crate) mod util;

pub type CommandHistory = VecDeque<String>;

pub struct Context<'a> {
    supports_keyboard_enhancement: bool,
    sessions: SessionManager<'a>,
    named_sessions: HashMap<String, usize>,
    variables: HashMap<String, String>,
    rctf_history: CommandHistory,
    termcraft_history: CommandHistory,
}

impl<'a> Context<'a> {
    pub fn new(
        rctf_history: Option<CommandHistory>,
        termcraft_history: Option<CommandHistory>,
    ) -> Result<Self> {
        Ok(Self {
            supports_keyboard_enhancement: crossterm::terminal::supports_keyboard_enhancement()?,
            sessions: SessionManager::new(), // TODO: restore sessions from files
            named_sessions: HashMap::new(),
            variables: HashMap::new(), // TODO: restore variables from files
            rctf_history: rctf_history.unwrap_or_default(),
            termcraft_history: termcraft_history.unwrap_or_default(),
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        terminal::setup(self.supports_keyboard_enhancement)?;
        let res = self.start_read_loop().await;
        terminal::teardown(self.supports_keyboard_enhancement)?;
        res
    }

    pub fn rctf_history(&self) -> &CommandHistory {
        &self.rctf_history
    }

    pub fn termcraft_history(&self) -> &CommandHistory {
        &self.termcraft_history
    }
}
