use std::collections::VecDeque;

use anyhow::Result;

pub(crate) mod constants;
pub mod files;
pub mod rctf;
mod ssh;
pub(crate) mod terminal;

#[derive(Clone, Debug, Default)]
pub struct Context {
    supports_keyboard_enhancement: bool,
    rctf_history: VecDeque<String>,
}

impl Context {
    pub fn new(rctf_history: Option<VecDeque<String>>) -> Result<Self> {
        Ok(Self {
            supports_keyboard_enhancement: crossterm::terminal::supports_keyboard_enhancement()?,
            rctf_history: rctf_history.unwrap_or_default(),
        })
    }

    pub fn rctf_history(&self) -> &VecDeque<String> {
        &self.rctf_history
    }

    pub async fn start(&mut self) -> Result<()> {
        terminal::setup(self.supports_keyboard_enhancement)?;
        let res = self.start_read_loop().await;
        terminal::teardown(self.supports_keyboard_enhancement)?;
        res
    }
}
