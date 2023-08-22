use std::collections::VecDeque;

use anyhow::Result;
use terminal::{setup_terminal, teardown_terminal};

pub(crate) mod constants;
pub mod rctf;
mod ssh;
pub(crate) mod terminal;

#[derive(Clone, Debug, Default)]
pub struct Context {
    supports_keyboard_enhancement: bool,
    rctf_history: VecDeque<String>,
}

impl Context {
    pub fn new() -> Result<Self> {
        Ok(Self {
            supports_keyboard_enhancement: crossterm::terminal::supports_keyboard_enhancement()?,
            ..Default::default()
        })
    }

    pub async fn start(mut self) -> Result<()> {
        setup_terminal(self.supports_keyboard_enhancement)?;
        let res = self.start_read_loop().await;
        teardown_terminal(self.supports_keyboard_enhancement)?;
        res
    }
}
