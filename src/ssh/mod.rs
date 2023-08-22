mod handler;

use std::{sync::Arc, fmt::Display};

use anyhow::{bail, Result};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use russh::{
    client::{self, Config, Handle, Msg},
    Channel, Disconnect, Pty, Sig,
};
use tokio::sync::oneshot;

use crate::{
    terminal::{setup_terminal, teardown_terminal},
    Context,
};

use self::handler::Handler;

const ETX: u8 = 3;
const EOT: u8 = 4;
const BACKSPACE: u8 = 8;

#[derive(Debug, Clone)]
enum Exit {
    Status(u32),
    Signal(Sig, String),
}

impl Display for Exit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Exit::Status(code) => write!(f, "Process exited with code {}.", code),
            Exit::Signal(signal, reason) => write!(f, "Process exited with signal SIG{:?}: {}", signal, reason),
        }
    }
}

impl Context {
    pub async fn start_ssh(self) -> Result<()> {
        let (tx_exit, rx_exit) = oneshot::channel();
        let session = self.create_session(Handler::new(tx_exit)).await?;
        let mut channel = session.channel_open_session().await?;
        channel
            .request_pty(
                true,
                "xterm",
                0,
                0,
                0,
                0,
                &[
                    (Pty::VINTR, ETX as u32),
                    (Pty::VEOF, EOT as u32),
                    (Pty::VERASE, BACKSPACE as u32),
                    (Pty::VEOL, b'\n'.into()),
                ],
            )
            .await?;
        channel.request_shell(true).await?;

        setup_terminal()?;

        self.start_read_loop(&mut channel, rx_exit).await?;

        teardown_terminal()?;
        channel.eof().await?;
        session
            .disconnect(Disconnect::ByApplication, "User exited.", "en")
            .await?;
        println!();

       Ok(())
    }

    async fn create_session(&self, handler: Handler) -> Result<Handle<Handler>> {
        let config = Arc::new(Config::default());
        let mut session = client::connect(
            config,
            (&self.ssh_settings.ip[..], self.ssh_settings.port),
            handler,
        )
        .await?;
        let authenticated = session
            .authenticate_password(&self.ssh_settings.username, &self.ssh_settings.password)
            .await?;

        if !authenticated {
            bail!("Failed to authenticate.");
        }

        Ok(session)
    }

    async fn start_read_loop(&self, channel: &mut Channel<Msg>, rx_exit: oneshot::Receiver<Exit>) -> Result<()> {
        let mut reader = EventStream::new();
        loop {
            let Some(event) = reader.next().await else {
                let exit = rx_exit.await?;
                if let Exit::Status(0) = exit {
                    return Ok(());
                } else {
                    bail!(exit);
                }
            };
            let event = event?;

            if event == Event::Key(KeyCode::Esc.into()) {
                return Ok(());
            }

            if let Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press | KeyEventKind::Repeat,
                ..
            }) = event
            {
                let data: &[u8] = match (code, modifiers) {
                    (KeyCode::Enter, _) => &[b'\n'],
                    (KeyCode::Backspace, _) => &[BACKSPACE],
                    (KeyCode::Tab, _) => &[b'\t'],
                    (KeyCode::Up, _) => b"\x1b[A",
                    (KeyCode::Down, _) => b"\x1b[B",
                    (KeyCode::Right, _) => b"\x1b[C",
                    (KeyCode::Left, _) => b"\x1b[D",
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => &[ETX],
                    (KeyCode::Char('d'), KeyModifiers::CONTROL) => &[EOT],
                    (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                        channel.data(&[c as u8][..]).await?;
                        continue;
                    }
                    _ => continue,
                };
                channel.data(&data[..]).await?;
            }
        }
    }
}
