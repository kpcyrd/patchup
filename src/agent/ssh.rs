use crate::errors::*;
use crate::ssh;
use russh::{
    Channel, Preferred,
    client::{Handle, Msg},
    keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKey},
};
use serde::Serialize;
use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

pub use crate::ssh::AGENT_USER;

pub enum ServerKeyVerification {
    Matches(PublicKey),
    Report(mpsc::Sender<PublicKey>),
}

pub struct SshClient {
    server_key_verification: ServerKeyVerification,
}

impl russh::client::Handler for SshClient {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        match &self.server_key_verification {
            ServerKeyVerification::Matches(expected) => {
                if server_public_key == expected {
                    debug!("Server public key matches expected key");
                    Ok(true)
                } else {
                    warn!(
                        "Server public key does NOT match expected key, received: {}",
                        server_public_key.to_string()
                    );
                    Ok(false)
                }
            }
            ServerKeyVerification::Report(sender) => {
                debug!(
                    "Detected server public key: {}",
                    server_public_key.to_string()
                );
                sender.try_send(server_public_key.clone())?;
                Ok(true)
            }
        }
    }
}

pub struct SshClientSession {
    session: russh::client::Handle<SshClient>,
}

impl SshClientSession {
    pub async fn exec(&self, command: &str) -> Result<Channel<Msg>> {
        let channel = self.session.channel_open_session().await?;
        channel.exec(true, command).await?;
        Ok(channel)
    }
}

pub async fn connect_anonymous(
    addr: SocketAddr,
    user: &str,
    server_key_verification: ServerKeyVerification,
) -> Result<Handle<SshClient>> {
    let config = russh::client::Config {
        client_id: ssh::ID,
        inactivity_timeout: Some(ssh::KEEPALIVE_INTERVAL * ssh::KEEPALIVE_MAX),
        preferred: Preferred {
            kex: Cow::Owned(vec![
                russh::kex::MLKEM768X25519_SHA256,
                russh::kex::CURVE25519_PRE_RFC_8731,
                russh::kex::EXTENSION_SUPPORT_AS_CLIENT,
            ]),
            ..Default::default()
        },
        ..<_>::default()
    }
    .into();
    let sh = SshClient {
        server_key_verification,
    };

    debug!("Connecting to ssh server at {addr:?} with user {user:?}");
    let session = russh::client::connect(config, addr, sh).await?;

    Ok(session)
}

pub async fn connect(
    addr: SocketAddr,
    user: &str,
    key: Arc<PrivateKey>,
    server_key: PublicKey,
) -> Result<SshClientSession> {
    let server_key_verification = ServerKeyVerification::Matches(server_key);
    let mut session = connect_anonymous(addr, user, server_key_verification).await?;

    let key = PrivateKeyWithHashAlg::new(key, Some(HashAlg::Sha256));
    debug!(
        "Authenticating with private key: {}",
        key.public_key().to_string()
    );
    session.authenticate_publickey(user, key).await?;
    debug!("Successfully setup SSH session");

    Ok(SshClientSession { session })
}

pub async fn submit_to_hub<T: Serialize>(
    addr: SocketAddr,
    key: Arc<PrivateKey>,
    server_key: PublicKey,
    data: &T,
) -> Result<()> {
    let ssh = connect(addr, ssh::AGENT_USER, key, server_key).await?;
    let channel = ssh.exec(ssh::AGENT_CMD).await?;
    let sshd_stream = channel.into_stream();
    let (mut _sshd_rx, mut sshd_tx) = tokio::io::split(sshd_stream);

    let mut buf = serde_json::to_string(data)?;
    buf.push('\n');
    sshd_tx.write_all(buf.as_bytes()).await?;

    /*
    let mut buf = String::new();
    sshd_rx.read_to_string(&mut buf).await?;
    info!("Received from hub: {buf:?}");
    */

    Ok(())
}
