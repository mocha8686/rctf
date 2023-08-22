mod handler;

use std::sync::Arc;

use anyhow::{Result, bail};
use crossterm::event::{EventStream, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use russh::{client::{Handle, self, Msg, Config}, Disconnect, Pty, Channel};
use serde::{Serialize, Deserialize};

use crate::terminal::{teardown_terminal, setup_terminal};

use self::handler::Handler;

const ETX: u8 = 3;
const EOT: u8 = 4;
const BACKSPACE: u8 = 8;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    ip: String,
    port: u16,
    username: String,
    password: String,
}

#[derive(Debug, Clone)]
pub struct Context {
    pub ssh_settings: Settings,
}

impl Context {
    pub async fn start(self) -> Result<()> {
        let session = self.create_session().await?;
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

        self.start_read_loop(&mut channel).await?;

        channel.eof().await?;
        session
            .disconnect(Disconnect::ByApplication, "User exited.", "en")
            .await?;
        teardown_terminal()?;
        println!();

        Ok(())
    }

    async fn create_session(&self) -> Result<Handle<Handler>> {
        let config = Arc::new(Config::default());
        let sh = Handler;
        let mut session = client::connect(
            config,
            (&self.ssh_settings.ip[..], self.ssh_settings.port),
            sh,
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

    async fn start_read_loop(&self, channel: &mut Channel<Msg>) -> Result<()> {
        let mut reader = EventStream::new();
        loop {
            let Some(event) = reader.next().await else {
                continue;
            };
            let event = event?;

            if event == Event::Key(KeyCode::Esc.into()) {
                break Ok(());
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
