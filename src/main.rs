use std::{io, str::FromStr, sync::Arc};

use anyhow::{bail, Result};
use async_trait::async_trait;
use config::Config;
use crossterm::{
    cursor,
    event::{self, Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue, style,
    terminal::{self, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use russh::{client, ChannelId};
use russh_keys::key;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

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

#[tokio::main]
async fn main() -> Result<()> {
    let settings: Settings = Config::builder()
        .add_source(config::Environment::with_prefix("RCTF"))
        .add_source(config::File::with_name("./rctf.ini"))
        .build()?
        .try_deserialize()?;

    let (tx_out, _) = mpsc::channel(5);

    let config = Arc::new(client::Config::default());
    let sh = Client(tx_out);
    let mut session = client::connect(config, (&settings.ip[..], settings.port), sh).await?;
    let authenticated = session
        .authenticate_password(&settings.username, &settings.password)
        .await?;

    if !authenticated {
        bail!("Failed to authenticate.");
    }

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
        style::Print("Hello, world!"),
        cursor::MoveToNextLine(1),
        cursor::SavePosition,
        cursor::MoveTo(0, rows),
        style::Print("$ "),
    )?;

    let mut reader = EventStream::new();
    let mut cmd = String::new();
    loop {
        let event = reader.next().await;

        match event {
            Some(Ok(event)) => {
                // println!("Event::{:?}\r", event);
                match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Esc,
                        kind: KeyEventKind::Press,
                        modifiers: _,
                        state: _,
                    })
                    | Event::Key(KeyEvent {
                        code: KeyCode::Char('c'),
                        kind: KeyEventKind::Press,
                        modifiers: KeyModifiers::CONTROL,
                        state: _,
                    }) => break,
                    Event::Key(KeyEvent {
                        code: KeyCode::Backspace,
                        kind: KeyEventKind::Press | KeyEventKind::Repeat,
                        modifiers: _,
                        state: _,
                    }) => {
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
                    Event::Key(KeyEvent {
                        code: KeyCode::Enter,
                        kind: KeyEventKind::Press,
                        modifiers: _,
                        state: _,
                    }) => {
                        execute!(
                            stdout,
                            cursor::RestorePosition,
                            style::Print(format!("$ {}", cmd)),
                            cursor::MoveToNextLine(1),
                            cursor::SavePosition,
                            cursor::MoveTo(2, rows),
                            terminal::Clear(terminal::ClearType::UntilNewLine),
                        )?;
                        cmd.clear();
                    }
                    Event::Key(KeyEvent {
                        code: KeyCode::Char(c),
                        kind: KeyEventKind::Press,
                        modifiers: _,
                        state: _,
                    }) => {
                        cmd.push(c);
                        execute!(stdout, style::Print(c))?;
                    }
                    _ => {}
                }
            }
            Some(Err(e)) => eprintln!("Error: {}\r", e),
            None => break,
        }
    }

    disable_raw_mode()?;

    todo!();

    // let mut stdout = io::stdout();
    // let mut channel = session.channel_open_session().await?;
    // channel.request_shell(true).await?;
    // if let Some(data) = rx_out.recv().await {
    //     stdout.write_all(&data).await?;
    //     stdout.write_all(b"$ ").await?;
    //     stdout.flush().await?;
    // }
    //
    // let (tx_in, mut rx_in) = mpsc::channel(5);
    //
    // loop {
    //     let Some(cmd) = rx_in.recv().await else {
    //         continue;
    //     };
    //     let cmd: Command = cmd.parse()?;
    //
    //     match cmd {
    //         Command::Exit => break,
    //         Command::Clear => todo!(),
    //         Command::Remote(cmd) => {
    //             channel.data(cmd.as_bytes()).await?;
    //
    //             if let Some(data) = rx_out.recv().await {
    //                 stdout.write_all(&data).await?;
    //             }
    //         }
    //     }
    //
    //     stdout.write_all(b"$ ").await?;
    //     stdout.flush().await?;
    // }
    //
    // Ok(())
}
