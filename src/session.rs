use std::borrow::Cow;

use anyhow::{bail, Result};
use async_trait::async_trait;
use tabled::Tabled;

use crate::{
    ssh::{SshSession, SshSettings},
    termcraft::TermcraftResponse,
    terminal::println,
    Context,
};

use self::stable_vec::StableVec;

mod stable_vec;

pub(crate) type SessionManager = StableVec<Box<dyn Session>>;

#[derive(Debug, Clone)]
pub(crate) enum SessionExit {
    Termcraft,
    Exit,
}

#[derive(Debug, Clone)]
pub(crate) enum SessionSelection {
    Index(usize),
    Name(String),
}

#[derive(Debug, Clone)]
pub(crate) enum SessionType {
    Ssh(SshSettings),
}

#[async_trait]
pub(crate) trait Session {
    fn type_name(&self) -> &'static str;

    async fn connect(&mut self) -> Result<()>;
    async fn start_read_loop(&mut self) -> Result<SessionExit>;
    async fn reset_prompt(&mut self) -> Result<()>;
    async fn send(&mut self, data: &[u8]) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;

    fn name(&self) -> Option<&str>;
    fn name_mut(&mut self) -> &mut String;

    fn index(&self) -> usize;
}

impl Context {
    pub(crate) async fn start_session(&mut self, session_type: SessionType) -> Result<()> {
        match session_type {
            SessionType::Ssh(settings) => {
                let session_index = self.sessions.next_index();
                let mut session = SshSession::new(settings, session_index);
                session.connect().await?;

                self.sessions.push(Box::new(session));
                self.handle_session(session_index).await?;
            }
        }

        Ok(())
    }

    pub(crate) async fn resume_session(
        &mut self,
        session_selection: SessionSelection,
    ) -> Result<()> {
        let session_index = match session_selection {
            SessionSelection::Index(index) => index,
            SessionSelection::Name(name) => {
                let Some(session_index) = self
                    .sessions
                    .iter()
                    .flatten()
                    .position(|session| session.name() == Some(&name))
                else {
                    bail!("No session found with name {name}.");
                };
                session_index
            }
        };

        self.handle_session(session_index).await?;

        Ok(())
    }

    async fn handle_session(&mut self, session_index: usize) -> Result<()> {
        loop {
            let res = {
                let Some(session) = self.sessions.get_mut(session_index) else {
                    bail!("No session found with index {session_index}.");
                };
                session.start_read_loop().await?
            };

            println("")?;

            match res {
                SessionExit::Termcraft => {
                    let res = self.start_termcraft(session_index).await?;
                    match res {
                        TermcraftResponse::Cmd(cmd) => {
                            let Some(session) = self.sessions.get_mut(session_index) else {
                                bail!("Could not find session with index {session_index}.");
                            };
                            session.reset_prompt().await?;
                            session.send(format!("{cmd}\n").as_bytes()).await?;
                            continue;
                        }
                        TermcraftResponse::Background => break,
                        TermcraftResponse::Exit => {
                            let Some(session) = self.sessions.get_mut(session_index) else {
                                bail!("Could not find session with index {session_index}.");
                            };
                            session.reset_prompt().await?;
                            continue;
                        }
                    }
                }
                SessionExit::Exit => {
                    let name = {
                        let Some(session) = self.sessions.get_mut(session_index) else {
                            bail!("Could not find session with index {session_index}.");
                        };
                        session.disconnect().await?;
                        session.name().map(|str| str.to_string())
                    };

                    self.sessions.remove(session_index);

                    if let Some(name) = name {
                        self.named_sessions.remove(&name);
                    }

                    break;
                }
            }
        }

        Ok(())
    }
}

impl Tabled for &dyn Session {
    const LENGTH: usize = 3;

    fn fields(&self) -> Vec<Cow<'_, str>> {
        [
            self.index().to_string(),
            self.type_name().to_string(),
            self.name().unwrap_or("").to_string(),
        ]
        .map(|name| Cow::Owned(name.to_string()))
        .to_vec()
    }

    fn headers() -> Vec<Cow<'static, str>> {
        ["Index", "Type", "Name"]
            .map(|header| Cow::Owned(header.to_string()))
            .to_vec()
    }
}
