use crate::errors::*;
use russh::{
    Channel, ChannelId, MethodKind, MethodSet, SshId,
    keys::{PrivateKey, PublicKey},
    server::{Auth, Msg, Server, Session},
};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(90);
pub const KEEPALIVE_MAX: u32 = 2;

pub struct SshServer {
    // shared: Arc<hub::Shared>,
}

impl SshServer {
    /*
    pub fn new(shared: Arc<hub::Shared>) -> Self {
        Self { shared }
    }
    */

    pub fn new() -> Self {
        Self {}
    }

    pub async fn run(&mut self, key: PrivateKey, bind: SocketAddr) -> Result<()> {
        let config = russh::server::Config {
            server_id: SshId::Standard("SSH-2.0-flowers-are-blooming-in-antarctica".into()),
            methods: MethodSet::from([MethodKind::PublicKey].as_slice()),
            keepalive_interval: Some(KEEPALIVE_INTERVAL),
            keepalive_max: KEEPALIVE_MAX as usize,
            auth_rejection_time: Duration::from_millis(250),
            auth_rejection_time_initial: Some(Duration::from_secs(0)),
            keys: vec![key],
            nodelay: true,
            ..Default::default()
        };

        info!("Starting SSH server on {bind}");
        self.run_on_address(Arc::new(config), bind).await?;
        bail!("SSH server has stopped unexpectedly")
    }
}

impl Server for SshServer {
    type Handler = SshSession;

    fn new_client(&mut self, _: Option<SocketAddr>) -> Self::Handler {
        // SshSession::new(self.shared.clone())
        SshSession::new()
    }
}

pub struct SshSession {
    pending_channels: BTreeMap<ChannelId, Channel<Msg>>,
}

impl SshSession {
    pub fn new() -> Self {
        Self {
            pending_channels: Default::default(),
        }
    }

    pub fn take_pending_channel(&mut self, channel_id: ChannelId) -> Option<Channel<Msg>> {
        self.pending_channels.remove(&channel_id)
    }
}

impl russh::server::Handler for SshSession {
    type Error = anyhow::Error;

    async fn auth_publickey(
        &mut self,
        _user: &str,
        _public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        info!("TODO: authenticate public key"); // TODO
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        debug!("Channel open session: {channel:?}");
        self.pending_channels.insert(channel.id(), channel);
        Ok(true)
    }

    async fn channel_close(&mut self, channel_id: ChannelId, _session: &mut Session) -> Result<()> {
        debug!("Client closed channel: {channel_id:?}");
        self.pending_channels.remove(&channel_id);
        // TODO: maybe shutdown active task (currently we use ChannelStream everywhere anyway)
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel_id: ChannelId,
        _session: &mut Session,
    ) -> std::result::Result<(), Self::Error> {
        // After a client has sent an EOF, indicating that they don't want
        // to send more data in this session, the channel can be closed.
        trace!("Client sent channel EOF");
        self.pending_channels.remove(&channel_id);
        Ok(())
    }

    /*
    async fn shell_request(
        &mut self,
        channel_id: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        info!("Requested shell session for channel {channel_id:?}");
        Ok(())
    }
    */

    async fn exec_request(
        &mut self,
        channel_id: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let Some(_channel) = self.take_pending_channel(channel_id) else {
            session.channel_failure(channel_id)?;
            return Ok(());
        };

        let Ok(cmd) = str::from_utf8(data) else {
            session.channel_failure(channel_id)?;
            return Ok(());
        };

        debug!("Request exec for channel {channel_id:?}: {cmd:?}");
        session.channel_success(channel_id)?;
        let handle = session.handle();

        match cmd {
            "_PATCHUP" => {
                let _ = handle.data(channel_id, "{\"status\":\"ok\"}\n").await;
                let _ = handle.exit_status_request(channel_id, 0).await;
                let _ = handle.eof(channel_id).await;
                let _ = handle.close(channel_id).await;
            }
            "ls" => {
                let _ = handle.data(channel_id, "Hello world!\n").await;
                let _ = handle.exit_status_request(channel_id, 0).await;
                let _ = handle.eof(channel_id).await;
                let _ = handle.close(channel_id).await;
            }
            _ => {
                let error_msg = format!("Refused to execute command: {cmd}\n");
                let _ = handle.extended_data(channel_id, 1, error_msg).await;
                let _ = handle.exit_status_request(channel_id, 1).await;
                let _ = handle.eof(channel_id).await;
                let _ = handle.close(channel_id).await;
            }
        }

        Ok(())
    }
}
