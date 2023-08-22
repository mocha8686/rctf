use std::collections::VecDeque;

use anyhow::Result;

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
        terminal::setup(self.supports_keyboard_enhancement)?;
        let res = self.start_read_loop().await;
        terminal::teardown(self.supports_keyboard_enhancement)?;
        res
    }
}
