use crate::errors::*;
use crate::ssh;
use russh::{
    Channel, Preferred,
    client::Msg,
    keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKey},
};
use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

struct SshClient {}

impl russh::client::Handler for SshClient {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        // TODO: actually verify the key
        debug!("TODO: Server public key: {}", server_public_key.to_string());
        Ok(true)
    }
}

struct SshClientSession {
    session: russh::client::Handle<SshClient>,
}

impl SshClientSession {
    pub async fn exec(&self, command: &str) -> Result<Channel<Msg>> {
        let channel = self.session.channel_open_session().await?;
        channel.exec(true, command).await?;
        Ok(channel)
    }
}

async fn connect(addr: SocketAddr, user: &str, key: Arc<PrivateKey>) -> Result<SshClientSession> {
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
    let sh = SshClient {};

    debug!("Connecting to ssh server at {addr:?} with user {user:?}");
    let mut session = russh::client::connect(config, addr, sh).await?;
    let key = PrivateKeyWithHashAlg::new(key, Some(HashAlg::Sha256));
    debug!(
        "Authenticating with private key: {}",
        key.public_key().to_string()
    );
    session.authenticate_publickey(user, key).await?;
    debug!("Successfully setup SSH session");

    Ok(SshClientSession { session })
}

pub async fn submit_to_hub(addr: SocketAddr, key: Arc<PrivateKey>) -> Result<()> {
    let ssh = connect(addr, "patchup", key).await?;
    let channel = ssh.exec("_PATCHUP").await?;
    let sshd_stream = channel.into_stream();
    let (mut sshd_rx, mut _sshd_tx) = tokio::io::split(sshd_stream);

    let mut buf = String::new();
    sshd_rx.read_to_string(&mut buf).await?;
    info!("Received from hub: {buf:?}");

    Ok(())
}
