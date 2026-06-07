use crate::agent::patches::UpdateStatus;
use crate::errors::*;
use crate::ipc;
use crate::node::NodeInfo;
use russh::keys::PublicKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use tokio::io::BufStream;
use tokio::net::UnixStream;

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Status,
    Refresh { mandatory: bool },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Status {
    pub ssh_key: PublicKey,
    pub node: NodeInfo,
    pub updates: Option<BTreeMap<String, UpdateStatus>>,
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
        let msg = ipc::recv(&mut self.stream).await?;
        Ok(msg)
    }
}
