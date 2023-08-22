mod handler;

use std::{fmt::Display, sync::Arc};

use anyhow::{bail, Result};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use russh::{
    client::{self, Config, Handle, Msg},
    Channel, Disconnect, Pty, Sig,
};
use tokio::{select, sync::mpsc};

use crate::{
    constants::{BACKSPACE, EOT, ETX},
    Context,
};

use self::handler::Handler;

#[derive(Debug, Clone)]
pub(crate) struct SshSettings {
    pub(crate) hostname: String,
    pub(crate) port: u16,
    pub(crate) username: String,
    pub(crate) password: String,
}

#[derive(Debug, Clone)]
enum Exit {
    Status(u32),
    Signal(Sig, String),
}

impl Display for Exit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Exit::Status(code) => write!(f, "Process exited with code {code}."),
            Exit::Signal(signal, reason) => {
                write!(f, "Process exited with signal SIG{signal:?}: {reason}")
            }
        }
    }
}

impl Context {
    pub(crate) async fn start_ssh(&mut self, settings: SshSettings) -> Result<()> {
        let (tx_exit, rx_exit) = mpsc::channel(1);
        let session = self.create_session(Handler::new(tx_exit), settings).await?;
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
                    (Pty::VINTR, ETX.into()),
                    (Pty::VEOF, EOT.into()),
                    (Pty::VERASE, BACKSPACE.into()),
                    (Pty::VEOL, b'\n'.into()),
                ],
            )
            .await?;
        channel.request_shell(true).await?;

        self.start_ssh_read_loop(&mut channel, rx_exit).await?;

        channel.eof().await?;
        session
            .disconnect(Disconnect::ByApplication, "User exited.", "en")
            .await?;
        println!();

        Ok(())
    }

    async fn create_session(
        &self,
        handler: Handler,
        settings: SshSettings,
    ) -> Result<Handle<Handler>> {
        let config = Arc::new(Config::default());
        let mut session =
            client::connect(config, (&settings.hostname[..], settings.port), handler).await?;
        let authenticated = session
            .authenticate_password(&settings.username, &settings.password)
            .await?;

        if !authenticated {
            bail!("Failed to authenticate.");
        }

        Ok(session)
    }

    async fn start_ssh_read_loop(
        &self,
        channel: &mut Channel<Msg>,
        mut rx_exit: mpsc::Receiver<Exit>,
    ) -> Result<()> {
        let mut reader = EventStream::new();
        loop {
            select! {
                event = reader.next() => {
                    let Some(event) = event else {
                        return Ok(());
                    };

                    if let Event::Key(KeyEvent {
                        code,
                        modifiers,
                        kind: KeyEventKind::Press | KeyEventKind::Repeat,
                        ..
                    }) = event?
                    {
                        let data: &[u8] = match (code, modifiers) {
                            (KeyCode::Esc, _) => return Ok(()),
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
                        channel.data(data).await?;
                    }
                }
                exit = rx_exit.recv() => {
                    let Some(exit) = exit else {
                        bail!("Failed to get exit status.");
                    };
                    if let Exit::Status(0) = exit {
                        return Ok(());
                    }

                    bail!(exit);
                }
            }
        }
    }
}
