use std::{io, str::FromStr, sync::Arc};

use anyhow::{bail, Result};
use async_trait::async_trait;
use config::Config;
use crossterm::{
    cursor,
    event::{self, Event, EventStream, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue, style,
    terminal::{self, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use russh::{client, ChannelId};
use russh_keys::key;
use serde::{Deserialize, Serialize};
use tokio::{select, sync::mpsc};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Settings {
    ip: String,
    port: u16,
    username: String,
    password: String,
}

#[derive(Clone, Debug)]
enum Command {
    Exit,
    Clear,
    Remote(String),
}

impl FromStr for Command {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim() {
            "exit" => Ok(Self::Exit),
            "clear" => Ok(Self::Clear),
            s => Ok(Self::Remote(format!("{}\n", s))),
        }
    }
}

struct Client(mpsc::Sender<Box<[u8]>>);

#[async_trait]
impl client::Handler for Client {
    type Error = anyhow::Error;

    async fn check_server_key(
        self,
        _server_public_key: &key::PublicKey,
    ) -> core::result::Result<(Self, bool), Self::Error> {
        Ok((self, true))
    }

    async fn data(
        self,
        _channel: ChannelId,
        data: &[u8],
        session: client::Session,
    ) -> core::result::Result<(Self, client::Session), Self::Error> {
        self.0.send(data.into()).await?;
        Ok((self, session))
    }
}

async fn start_terminal(
    tx_stdin: mpsc::Sender<String>,
    mut rx_stdout: mpsc::Receiver<Box<[u8]>>,
) -> Result<()> {
    let mut stdout = io::stdout();

    if let Ok(true) = terminal::supports_keyboard_enhancement() {
        queue!(
            stdout,
            event::PushKeyboardEnhancementFlags(
                event::KeyboardEnhancementFlags::REPORT_EVENT_TYPES,
            )
        )?;
    }

    enable_raw_mode()?;
    let (_, rows) = terminal::size()?;
    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0),
        cursor::SavePosition,
        cursor::MoveTo(0, rows),
        style::Print("$ "),
    )?;

    let mut reader = EventStream::new();
    let mut cmd = String::new();
    loop {
        select! {
            event = reader.next() => {
                match event {
                    Some(Ok(Event::Key(e))) => {
                        match (e.code, e.kind, e.modifiers) {
                            (KeyCode::Esc, KeyEventKind::Press, _) | (KeyCode::Char('c'), KeyEventKind::Press, KeyModifiers::CONTROL) => break,
                            (KeyCode::Backspace, KeyEventKind::Press | KeyEventKind::Repeat, _) => {
                                if cmd.is_empty() {
                                    continue;
                                }
                                cmd.pop();
                                execute!(
                                    stdout,
                                    cursor::MoveLeft(1),
                                    terminal::Clear(terminal::ClearType::UntilNewLine),
                                )?;
                            }
                            (KeyCode::Enter, KeyEventKind::Press | KeyEventKind::Repeat, _) => {
                                execute!(
                                    stdout,
                                    cursor::RestorePosition,
                                    style::Print(format!("$ {}", cmd)),
                                    cursor::MoveToNextLine(1),
                                    cursor::SavePosition,
                                    cursor::MoveTo(2, rows),
                                    terminal::Clear(terminal::ClearType::UntilNewLine),
                                )?;

                                {
                                    let cmd: Command = cmd.parse()?;
                                    match cmd {
                                        Command::Exit => break,
                                        Command::Clear => execute!(stdout, terminal::Clear(terminal::ClearType::FromCursorUp))?,
                                        Command::Remote(cmd) => tx_stdin.send(cmd).await?,
                                    }
                                }

                                cmd.clear();
                            }
                            (KeyCode::Char(c), KeyEventKind::Press | KeyEventKind::Repeat, _) => {
                                cmd.push(c);
                                execute!(stdout, style::Print(c))?;
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(_)) => {},
                    Some(Err(e)) => bail!(e),
                    None => break,
                }
            }
            output = rx_stdout.recv() => {
                let Some(output) = output else {
                    break;
                };
                let output = std::str::from_utf8(&output)?;

                disable_raw_mode()?;
                execute!(
                    stdout,
                    cursor::RestorePosition,
                    style::Print(output),
                    cursor::MoveToNextLine(1),
                    cursor::SavePosition,
                    cursor::MoveTo(2 + cmd.len() as u16, rows),
                    terminal::Clear(terminal::ClearType::UntilNewLine),
                )?;
                enable_raw_mode()?;
            }
        }
    }

    disable_raw_mode()?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings: Settings = Config::builder()
        .add_source(config::Environment::with_prefix("RCTF"))
        .add_source(config::File::with_name("./rctf.ini"))
        .build()?
        .try_deserialize()?;

    let (tx_stdout, rx_stdout) = mpsc::channel(5);

    let config = Arc::new(client::Config::default());
    let sh = Client(tx_stdout);
    let mut session = client::connect(config, (&settings.ip[..], settings.port), sh).await?;
    let authenticated = session
        .authenticate_password(&settings.username, &settings.password)
        .await?;

    if !authenticated {
        bail!("Failed to authenticate.");
    }

    let (tx_stdin, mut rx_stdin) = mpsc::channel(5);

    let mut terminal_handle = tokio::spawn(async move {
        if let Err(e) = start_terminal(tx_stdin, rx_stdout).await {
            eprintln!("{}", e);
        };
    });

    let mut channel = session.channel_open_session().await?;
    channel.request_shell(true).await?;

    loop {
        select! {
            cmd = rx_stdin.recv() => {
                let Some(cmd) = cmd else {
                    continue;
                };
                channel.data(cmd.as_bytes()).await?;
            }
            res = &mut terminal_handle => {
                res?;
                break;
            }
        }
    }

    Ok(())
}
