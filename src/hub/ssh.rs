use crate::errors::*;
use crate::hub;
use crate::node::NodeInfo;
use crate::ssh;
use russh::{
    Channel, ChannelId, MethodKind, MethodSet,
    keys::{PrivateKey, PublicKey},
    server::{Auth, Msg, Server, Session},
};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader};

pub struct SshServer {
    shared: Arc<hub::Shared>,
}

impl SshServer {
    pub fn new(shared: Arc<hub::Shared>) -> Self {
        Self { shared }
    }

    pub async fn run(&mut self, key: PrivateKey, bind: SocketAddr) -> Result<()> {
        let config = russh::server::Config {
            server_id: ssh::ID,
            methods: MethodSet::from([MethodKind::PublicKey].as_slice()),
            keepalive_interval: Some(ssh::KEEPALIVE_INTERVAL),
            keepalive_max: ssh::KEEPALIVE_MAX as usize,
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
        SshSession::new(self.shared.clone())
    }
}

pub struct SshSession {
    shared: Arc<hub::Shared>,
    public_key: Option<PublicKey>,
    admin: bool,
    pending_channels: BTreeMap<ChannelId, Channel<Msg>>,
}

impl SshSession {
    pub fn new(shared: Arc<hub::Shared>) -> Self {
        Self {
            shared,
            public_key: None,
            admin: false,
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
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        let config = &self.shared.state.load().config;

        let is_admin = config
            .admins
            .iter()
            .any(|admin| admin.keys.contains(public_key));
        let is_agent = config
            .nodes
            .iter()
            .any(|node| node.keys.contains(public_key));

        if is_admin {
            info!(
                "SSH client authenticated as admin with public key: {}",
                public_key.to_string()
            );
            self.public_key = Some(public_key.clone());
            self.admin = true;
            Ok(Auth::Accept)
        } else if is_agent {
            info!(
                "SSH client authenticated as agent with public key: {}",
                public_key.to_string()
            );
            self.public_key = Some(public_key.clone());
            Ok(Auth::Accept)
        } else {
            debug!(
                "Rejected SSH client with unknown public key: {}",
                public_key.to_string()
            );
            Ok(Auth::reject())
        }
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

    async fn pty_request(
        &mut self,
        channel_id: ChannelId,
        _term: &str,
        _col_width: u32,
        _row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        info!("Requested PTY for channel {channel_id:?}");
        session.channel_failure(channel_id)?;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel_id: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        info!("Requested shell session for channel {channel_id:?}");
        session.channel_failure(channel_id)?;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel_id: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let Some(channel) = self.take_pending_channel(channel_id) else {
            session.channel_failure(channel_id)?;
            return Ok(());
        };

        let Some(public_key) = &self.public_key else {
            session.channel_failure(channel_id)?;
            return Ok(());
        };

        let Ok(cmd) = str::from_utf8(data) else {
            session.channel_failure(channel_id)?;
            return Ok(());
        };

        let Ok(cmd) = shell_words::split(cmd) else {
            session.channel_failure(channel_id)?;
            return Ok(());
        };

        debug!("Request exec for channel {channel_id:?}: {cmd:?}");
        session.channel_success(channel_id)?;
        let handle = session.handle();

        match cmd.first().map(String::as_str) {
            Some(ssh::AGENT_CMD) => {
                let shared = self.shared.clone();
                let public_key = public_key.clone();

                tokio::spawn(async move {
                    let stream = channel.into_stream();
                    if let Err(err) = agent_stream(shared, public_key, stream).await {
                        error!("Error in hub agent session: {err:#}");
                    }
                });
            }
            Some("ls") if self.admin => {
                let state = self.shared.state.load();

                if cmd.iter().any(|arg| arg == "--json") {
                    let mut buf = serde_json::to_string_pretty(&state.nodes)?;
                    buf.push('\n');
                    let _ = handle.data(channel_id, buf).await;
                } else {
                    for (public_key, agent) in &state.nodes {
                        let node = &agent.nodeinfo;
                        let elapsed = humantime::format_duration(agent.last_ping.elapsed());
                        let mut buf = format!("{}\n", public_key.to_string());
                        buf.push_str(&format!("  hostname:  {}\n", node.hostname));
                        buf.push_str(&format!("  os:        {}\n", node.os));
                        buf.push_str(&format!("  arch:      {}\n", node.arch));
                        buf.push_str(&format!("  kernel:    {}\n", node.kernel));
                        buf.push_str(&format!("  last seen: {elapsed} ago\n"));
                        if let Some(updates) = &node.updates {
                            if updates.is_empty() {
                                buf.push_str("  updates:   no package manager detected\n");
                            } else {
                                buf.push_str("  updates:\n");
                                for (manager, status) in updates {
                                    let n = status.pending.len();
                                    let nomen = if n == 1 { "update" } else { "updates" };
                                    let hint = if status.refresh_error {
                                        " (failed to refresh)"
                                    } else {
                                        ""
                                    };
                                    buf.push_str(&format!(
                                        "    {:<10} {n} pending {nomen}{hint}\n",
                                        format!("{manager}:"),
                                    ));
                                    for update in &status.pending {
                                        buf.push_str(&format!(
                                            "               - {}\n",
                                            update.name
                                        ));
                                    }
                                }
                            }
                        } else {
                            buf.push_str("  updates:   waiting for privileged process\n");
                        }
                        buf.push('\n');
                        let _ = handle.data(channel_id, buf).await;
                    }
                }

                let _ = handle.exit_status_request(channel_id, 0).await;
                let _ = handle.eof(channel_id).await;
                let _ = handle.close(channel_id).await;
            }
            _ => {
                let error_msg = format!("Refused to execute command: {cmd:?}\n");
                let _ = handle.extended_data(channel_id, 1, error_msg).await;
                let _ = handle.exit_status_request(channel_id, 1).await;
                let _ = handle.eof(channel_id).await;
                let _ = handle.close(channel_id).await;
            }
        }

        Ok(())
    }
}

async fn agent_stream<S: AsyncRead + AsyncWrite + Unpin>(
    shared: Arc<hub::Shared>,
    public_key: PublicKey,
    stream: S,
) -> Result<()> {
    let (reader, mut _writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);

    let mut buf = String::new();
    reader.read_line(&mut buf).await?;

    let nodeinfo = serde_json::from_str::<NodeInfo>(&buf)?;
    debug!("Received nodeinfo from agent: {nodeinfo:?}");

    shared.ping_from_node(public_key.clone(), nodeinfo).await?;

    // writer.write_all(b"{\"status\":\"ok\"}\n").await?;
    Ok(())
}
