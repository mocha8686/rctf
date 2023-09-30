use async_trait::async_trait;
use russh::{
    client::{Handler as RusshHandler, Session},
    ChannelId, Disconnect, Sig,
};
use russh_keys::key;
use tokio::sync::{mpsc, watch};

use super::Exit;

pub(super) struct Handler {
    tx_exit: mpsc::Sender<Exit>,
    tx_stdout: watch::Sender<Vec<u8>>,
    tx_stderr: watch::Sender<Vec<u8>>,
}

impl Handler {
    pub(super) fn new(
        tx_exit: mpsc::Sender<Exit>,
        tx_stdout: watch::Sender<Vec<u8>>,
        tx_stderr: watch::Sender<Vec<u8>>,
    ) -> Self {
        Self {
            tx_exit,
            tx_stdout,
            tx_stderr,
        }
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
        self.tx_stdout.send(data.to_vec())?;
        Ok((self, session))
    }

    async fn extended_data(
        self,
        _channel: ChannelId,
        _ext: u32,
        data: &[u8],
        session: Session,
    ) -> core::result::Result<(Self, Session), Self::Error> {
        self.tx_stderr.send(data.to_vec())?;
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
        self.tx_exit.send(Exit::Status(exit_status)).await.ok();
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
        self.tx_exit
            .send(Exit::Signal(signal_name, error_message.to_string()))
            .await
            .ok();
        Ok((self, session))
    }
}
