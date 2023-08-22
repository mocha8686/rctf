use async_trait::async_trait;
use russh::{
    client::{Handler as RusshHandler, Session},
    ChannelId, Disconnect, Sig,
};
use russh_keys::key;
use tokio::{
    io::{self, AsyncWriteExt},
    sync::oneshot,
};

use crate::terminal::teardown_terminal;

use super::Exit;

pub(super) struct Handler(Option<oneshot::Sender<Exit>>);

impl Handler {
    pub(super) fn new(tx_exit: oneshot::Sender<Exit>) -> Self {
        Self(Some(tx_exit))
    }
}

#[async_trait]
impl RusshHandler for Handler {
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
        session: Session,
    ) -> core::result::Result<(Self, Session), Self::Error> {
        let mut stdout = io::stdout();
        stdout.write_all(data).await?;
        stdout.flush().await?;
        Ok((self, session))
    }

    async fn extended_data(
        self,
        _channel: ChannelId,
        _ext: u32,
        data: &[u8],
        session: Session,
    ) -> core::result::Result<(Self, Session), Self::Error> {
        let mut stderr = io::stderr();
        stderr.write_all(data).await?;
        stderr.flush().await?;
        Ok((self, session))
    }

    async fn exit_status(
        mut self,
        channel: ChannelId,
        exit_status: u32,
        mut session: Session,
    ) -> core::result::Result<(Self, Session), Self::Error> {
        session.eof(channel);
        session.disconnect(
            Disconnect::ByApplication,
            "Process exited with status.",
            "en",
        );
        teardown_terminal()?;
        self.0.take().unwrap().send(Exit::Status(exit_status)).ok();
        Ok((self, session))
    }

    async fn exit_signal(
        mut self,
        channel: ChannelId,
        signal_name: Sig,
        _core_dumped: bool,
        error_message: &str,
        _lang_tag: &str,
        mut session: Session,
    ) -> core::result::Result<(Self, Session), Self::Error> {
        session.eof(channel);
        session.disconnect(
            Disconnect::ByApplication,
            "Process exited with signal.",
            "en",
        );
        teardown_terminal()?;
        self.0
            .take()
            .unwrap()
            .send(Exit::Signal(signal_name, error_message.to_string()))
            .ok();
        Ok((self, session))
    }
}
