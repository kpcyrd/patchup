pub mod config;
mod metrics;
mod ssh;

use crate::args::Hub;
use crate::errors::*;
use crate::keygen;
use crate::node::NodeInfo;
use arc_swap::ArcSwap;
use russh::keys::PublicKey;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Instant;

const CHANNEL_BACKLOG: usize = 64;

#[derive(Debug, Clone)]
struct State {
    nodes: BTreeMap<PublicKey, Agent>,
}

impl State {
    pub fn new() -> Self {
        Self {
            nodes: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Agent {
    nodeinfo: NodeInfo,
    #[serde(skip)]
    last_ping: Instant,
}

impl Agent {
    pub fn new(nodeinfo: NodeInfo) -> Self {
        Self {
            nodeinfo,
            last_ping: Instant::now(),
        }
    }
}

#[derive(Debug, Clone)]
struct Shared {
    state: Arc<ArcSwap<State>>,
    tx: mpsc::Sender<TaskEvent>,
}

impl Shared {
    pub fn new(state: Arc<ArcSwap<State>>) -> (Self, mpsc::Receiver<TaskEvent>) {
        let (tx, rx) = mpsc::channel(CHANNEL_BACKLOG);
        (Self { state, tx }, rx)
    }

    pub async fn ping_from_node(&self, public_key: PublicKey, nodeinfo: NodeInfo) -> Result<()> {
        self.tx
            .send(TaskEvent::PingNode {
                public_key,
                nodeinfo,
            })
            .await
            .context("Failed to record ping from node")
    }
}

#[derive(Debug)]
enum TaskEvent {
    PingNode {
        public_key: PublicKey,
        nodeinfo: NodeInfo,
    },
}

async fn state_machine(
    state: Arc<ArcSwap<State>>,
    mut rx: mpsc::Receiver<TaskEvent>,
) -> Result<()> {
    loop {
        let Some(msg) = rx.recv().await else {
            break Ok(());
        };
        info!("State machine woke up: {msg:?}");
        match msg {
            TaskEvent::PingNode {
                public_key,
                nodeinfo,
            } => {
                debug!("Ping from node: {public_key:?}");
                let mut new = state.load().as_ref().clone();
                match new.nodes.entry(public_key) {
                    Entry::Occupied(mut entry) => {
                        let entry = entry.get_mut();
                        entry.nodeinfo = nodeinfo;
                        entry.last_ping = Instant::now();
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(Agent::new(nodeinfo));
                    }
                }
                state.store(Arc::new(new));
            }
        }
    }
}

pub async fn run(_config: Option<&Path>, args: &Hub) -> Result<()> {
    let ssh_key_path = args.data.join("ssh.key");
    let ssh_key = keygen::init_from_path(&ssh_key_path).await?;

    let state = Arc::new(ArcSwap::from_pointee(State::new()));
    let (shared, rx) = Shared::new(state.clone());
    let shared = Arc::new(shared);
    let mut server = ssh::SshServer::new(shared.clone());

    tokio::select! {
        res = metrics::start(args.metrics, shared) => res,
        res = state_machine(state, rx) => res,
        res = server.run(ssh_key.clone(), args.bind) => res,
    }
}
