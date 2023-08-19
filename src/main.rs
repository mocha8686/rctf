use std::{str::FromStr, sync::Arc};

use anyhow::{bail, Result};
use async_trait::async_trait;
use config::Config;
use russh::{client, ChannelId};
use russh_keys::key;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
    spawn,
    sync::mpsc,
};

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

    let (tx_out, mut rx_out) = mpsc::channel(5);

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
    let mut channel = session.channel_open_session().await?;
    channel.request_shell(true).await?;
    if let Some(data) = rx_out.recv().await {
        stdout.write_all(&data).await?;
        stdout.write_all(b"$ ").await?;
        stdout.flush().await?;
    }

    let (tx_in, mut rx_in) = mpsc::channel(5);

    spawn(async move {
        let mut stdin = BufReader::new(io::stdin());
        loop {
            let mut input = String::new();
            if let Err(e) = stdin.read_line(&mut input).await {
                eprintln!("{}", e);
            }
            if let Err(e) = tx_in.send(input).await {
                eprintln!("{}", e);
            }
        }
    });

    loop {
        let Some(cmd) = rx_in.recv().await else {
            continue;
        };
        let cmd: Command = cmd.parse()?;

        match cmd {
            Command::Exit => break,
            Command::Clear => clearscreen::clear()?,
            Command::Remote(cmd) => {
                channel.data(cmd.as_bytes()).await?;

                if let Some(data) = rx_out.recv().await {
                    stdout.write_all(&data).await?;
                }
            }
        }

        stdout.write_all(b"$ ").await?;
        stdout.flush().await?;
    }

    Ok(())
}
