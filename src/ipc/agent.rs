use crate::agent;
use crate::agent::patches::{self, UpdateStatus};
use crate::errors::*;
use crate::ipc;
use crate::node::NodeInfo;
use russh::keys::PublicKey;
use serde::{Deserialize, Serialize};
use serde_with::{DurationSeconds, serde_as};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;
use tokio::io::BufStream;
use tokio::net::UnixStream;

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Status,
    Refresh { mandatory: bool },
    ConnectHub { hub: ipc::agent::Hub },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum OfferRequest {
    ListPkgBackends,
    QueryPkgBackend { name: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Status {
    pub ssh_key: PublicKey,
    pub node: NodeInfo,
    pub timers: Timers,
    pub updates: Option<BTreeMap<String, UpdateStatus>>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct Timers {
    #[serde_as(as = "DurationSeconds<u64>")]
    pub agent_uptime: Duration,
    #[serde_as(as = "Option<DurationSeconds<u64>>")]
    pub last_refresh: Option<Duration>,
    #[serde_as(as = "Option<DurationSeconds<u64>>")]
    pub last_refresh_offer: Option<Duration>,
}

impl Timers {
    pub fn refresh_offer_overdue(&self) -> bool {
        if let Some(last_refresh_offer) = self.last_refresh_offer {
            last_refresh_offer > agent::PATCH_REFRESH_INTERVAL + agent::OFFER_DEADLINE
        } else {
            self.agent_uptime > agent::OFFER_DEADLINE
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hub {
    pub addr: SocketAddr,
    pub server_key: PublicKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubConnected {
    pub error: Option<String>,
}

pub struct AgentIpc {
    stream: BufStream<UnixStream>,
}

impl AgentIpc {
    pub async fn connect<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        debug!("Connecting to agent socket at {:?}", path);
        let stream = UnixStream::connect(path)
            .await
            .with_context(|| format!("Failed to connect to socket: {:?}", path))?;
        debug!("Successfully opened socket connection");
        let stream = BufStream::new(stream);
        Ok(Self { stream })
    }

    pub async fn status(&mut self) -> Result<Status> {
        ipc::send(&mut self.stream, &Request::Status).await?;
        let msg = ipc::recv(&mut self.stream).await?;
        Ok(msg)
    }

    pub async fn offer_refresh(&mut self, mandatory: bool) -> Result<()> {
        ipc::send(&mut self.stream, &Request::Refresh { mandatory }).await?;

        while let Some(msg) = ipc::recv_opt::<_, OfferRequest>(&mut self.stream).await? {
            match msg {
                OfferRequest::ListPkgBackends => {
                    let backends = [
                        (patches::apk::ID, patches::apk::detect().await),
                        (patches::apt::ID, patches::apt::detect().await),
                    ]
                    .into_iter()
                    .filter(|(_, detected)| *detected)
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>();

                    debug!("Offering detected pkg backends: {backends:?}");
                    ipc::send(&mut self.stream, &backends).await?;
                }
                OfferRequest::QueryPkgBackend { name } => {
                    let updates = match name.as_str() {
                        patches::apk::ID => patches::apk::query().await?,
                        patches::apt::ID => patches::apt::query().await?,
                        _ => break,
                    };
                    ipc::send(&mut self.stream, &updates).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn connect_hub(&mut self, hub: Hub) -> Result<()> {
        ipc::send(&mut self.stream, &Request::ConnectHub { hub }).await?;
        let connected = ipc::recv::<_, HubConnected>(&mut self.stream).await?;
        if let Some(err) = connected.error {
            bail!("Hub connection failed: {err:?}");
        } else {
            Ok(())
        }
    }
}
