use std::{str::FromStr, sync::Arc};

use anyhow::{bail, Result};
use async_trait::async_trait;
use config::Config;
use crossterm::{
    event::{
        self, Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
        KeyboardEnhancementFlags,
    },
    execute,
    terminal::{self, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use russh::{client, ChannelId, Pty};
use russh_keys::key;
use serde::{Deserialize, Serialize};
use tokio::io::{self, AsyncWriteExt};

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

struct Client;

const ETX: u8 = 3;
const EOT: u8 = 4;
const BACKSPACE: u8 = 8;

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
        let mut stdout = io::stdout();
        stdout.write(data).await?;
        stdout.flush().await?;
        Ok((self, session))
    }

    async fn extended_data(
        self,
        _channel: ChannelId,
        _ext: u32,
        data: &[u8],
        session: client::Session,
    ) -> core::result::Result<(Self, client::Session), Self::Error> {
        let mut stderr = io::stderr();
        stderr.write(data).await?;
        stderr.flush().await?;
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

    let config = Arc::new(client::Config::default());
    let sh = Client;
    let mut session = client::connect(config, (&settings.ip[..], settings.port), sh).await?;
    let authenticated = session
        .authenticate_password(&settings.username, &settings.password)
        .await?;

    if !authenticated {
        bail!("Failed to authenticate.");
    }

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

    enable_raw_mode()?;

    if let Ok(true) = terminal::supports_keyboard_enhancement() {
        execute!(
            std::io::stdout(),
            event::PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )?;
    }

    let mut reader = EventStream::new();

    loop {
        let Some(event) = reader.next().await else {
            break;
        };
        let event = event?;

        if event == Event::Key(KeyCode::Esc.into()) {
            break;
        }

        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press | KeyEventKind::Repeat,
            ..
        }) = event
        {
            let data: Box<[u8]> = match (code, modifiers) {
                (KeyCode::Enter, _) => [b'\n'].into(),
                (KeyCode::Backspace, _) => [BACKSPACE].into(),
                (KeyCode::Tab, _) => [b'\t'].into(),
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => [ETX].into(),
                (KeyCode::Char('d'), KeyModifiers::CONTROL) => [EOT].into(),
                (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => [c as u8].into(),
                _ => continue,
            };
            channel.data(&data[..]).await?;
        }
    }

    disable_raw_mode()?;

    Ok(())
}
