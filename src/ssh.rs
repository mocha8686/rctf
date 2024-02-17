use std::{fmt::Display, sync::Arc};

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use russh::{
    client::{self, Config, Handle, Msg},
    Channel, Disconnect, Pty, Sig,
};
use tokio::{
    io::AsyncWriteExt,
    select,
    sync::{mpsc, watch},
};

use crate::session::{Session, SessionExit};

mod handler;
use handler::Handler;

pub const ETX: u8 = 3;
pub const EOT: u8 = 4;
pub const BACKSPACE: u8 = 8;

#[derive(Debug, Clone)]
pub struct SshSettings {
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
enum Exit {
    Status(u32),
    Signal(Sig, String),
}

enum Status {
    Disconnected,
    Connected {
        session: Handle<Handler>,
        channel: Channel<Msg>,
        rx_exit: mpsc::Receiver<Exit>,
        rx_stdout: watch::Receiver<Vec<u8>>,
        rx_stderr: watch::Receiver<Vec<u8>>,
    },
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

pub struct SshSession {
    hostname: String,
    port: u16,
    username: String,
    password: String,
    status: Status,
    name: String,
    index: usize,
}

impl SshSession {
    pub fn new(settings: SshSettings) -> Self {
        Self {
            hostname: settings.hostname,
            port: settings.port,
            username: settings.username,
            password: settings.password,
            status: Status::Disconnected,
            name: String::new(),
            index,
        }
    }

    async fn create_session(&self, handler: Handler) -> Result<Handle<Handler>> {
        let config = Arc::new(Config::default());
        let mut session = client::connect(config, (&self.hostname[..], self.port), handler).await?;
        let authenticated = session
            .authenticate_password(&self.username, &self.password)
            .await?;

        if !authenticated {
            bail!("Failed to authenticate.");
        }

        Ok(session)
    }
}

#[async_trait]
impl Session for SshSession {
    fn type_name(&self) -> &'static str {
        "Ssh"
    }

    async fn connect(&mut self) -> Result<()> {
        let (tx_exit, rx_exit) = mpsc::channel(1);
        let (tx_stdout, rx_stdout) = watch::channel(vec![]);
        let (tx_stderr, rx_stderr) = watch::channel(vec![]);

        let session = self
            .create_session(Handler::new(tx_exit, tx_stdout, tx_stderr))
            .await?;
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

        self.status = Status::Connected {
            session,
            channel,
            rx_exit,
            rx_stdout,
            rx_stderr,
        };

        Ok(())
    }

    async fn start_read_loop(&mut self) -> Result<SessionExit> {
        let Status::Connected {
            ref mut channel,
            ref mut rx_exit,
            ref rx_stdout,
            ref rx_stderr,
            ..
        } = self.status
        else {
            bail!("Cannot start read loop before connecting");
        };

        let print_loop_handle = {
            let mut rx_stdout = rx_stdout.clone();
            let mut rx_stderr = rx_stderr.clone();

            tokio::spawn(async move {
                loop {
                    select! {
                        res = rx_stdout.changed() => {
                            if let Err(_) = res {
                                break;
                            }

                            let msg = rx_stdout.borrow_and_update().clone();
                            let mut stdout = tokio::io::stdout();
                            stdout.write(&msg).await.ok();
                            stdout.flush().await.ok();
                        }
                        res = rx_stderr.changed() => {
                            if let Err(_) = res {
                                break;
                            }

                            let msg = rx_stdout.borrow_and_update().clone();
                            let mut stderr = tokio::io::stderr();
                            stderr.write(&msg).await.ok();
                            stderr.flush().await.ok();
                        }
                    }
                }
            })
        };

        let mut reader = EventStream::new();
        let res = loop {
            select! {
                event = reader.next() => {
                    let Some(event) = event else {
                        bail!("Out of events.");
                    };

                    if let Event::Key(KeyEvent {
                        code,
                        modifiers,
                        kind: KeyEventKind::Press | KeyEventKind::Repeat,
                        ..
                    }) = event?
                    {
                        let data: &[u8] = match (code, modifiers) {
                            (KeyCode::Esc, _) => break Ok(SessionExit::Termcraft),
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
                        break Err(anyhow!("Failed to get exit status."));
                    };
                    if let Exit::Status(0) = exit {
                        break Ok(SessionExit::Exit);
                    }

                    break Err(anyhow!(exit));
                }
            }
        };

        print_loop_handle.abort();
        print_loop_handle.await.ok();

        res
    }

    async fn reset_prompt(&mut self) -> Result<()> {
        let Status::Connected {
            ref mut channel,
            ref mut rx_stdout,
            ref mut rx_stderr,
            ..
        } = self.status
        else {
            bail!("Cannot send data before connecting");
        };
        channel.data(&[ETX][..]).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        rx_stdout.borrow_and_update();
        rx_stderr.borrow_and_update();
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<()> {
        let Status::Connected {
            ref mut channel, ..
        } = self.status
        else {
            bail!("Cannot send data before connecting");
        };
        channel.data(data).await?;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        let Status::Connected {
            ref session,
            ref mut channel,
            ..
        } = self.status
        else {
            return Ok(());
        };

        channel.eof().await?;
        session
            .disconnect(Disconnect::ByApplication, "User exited.", "en")
            .await?;
        println!();

        self.status = Status::Disconnected;

        Ok(())
    }

    fn name(&self) -> Option<&str> {
        if self.name.is_empty() {
            None
        } else {
            Some(&self.name)
        }
    }

    fn name_mut(&mut self) -> &mut String {
        &mut self.name
    }

    fn index(&self) -> usize {
        self.index
    }
}
