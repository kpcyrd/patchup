pub mod config;
mod metrics;
mod ssh;

use crate::args::Hub;
use crate::errors::*;
use crate::keygen;
use crate::node::NodeInfo;
use crate::signals;
use arc_swap::ArcSwap;
use russh::keys::PublicKey;
use serde::Serialize;
use std::collections::{BTreeMap, btree_map::Entry};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Instant;

const CHANNEL_BACKLOG: usize = 64;
const DEFAULT_BIND_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 2424);

#[derive(Debug, Clone)]
struct State {
    config: config::Config,
    nodes: BTreeMap<PublicKey, Agent>,
}

impl State {
    pub fn new(config: config::Config) -> Self {
        Self {
            config,
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
                nodeinfo: Box::new(nodeinfo),
            })
            .await
            .context("Failed to record ping from node")
    }
}

#[derive(Debug, Clone)]
enum TaskEvent {
    PingNode {
        public_key: PublicKey,
        nodeinfo: Box<NodeInfo>,
    },
    ReloadConfig(Option<PathBuf>),
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
                        entry.nodeinfo = *nodeinfo;
                        entry.last_ping = Instant::now();
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(Agent::new(*nodeinfo));
                    }
                }
                state.store(Arc::new(new));
            }
            TaskEvent::ReloadConfig(path) => match config::Config::load(path.as_deref()).await {
                Ok(config) => {
                    info!("Config reloaded successfully");
                    let mut new = state.load().as_ref().clone();
                    new.config = config;
                    // TODO: we may want to do some cleanup here, e.g. agents that are no longer valid after the config change
                    state.store(Arc::new(new));
                }
                Err(err) => {
                    error!("Failed to reload config: {err:#}");
                }
            },
        }
    }
}

pub async fn run(config_path: Option<PathBuf>, args: &Hub) -> Result<()> {
    let config = config::Config::load(config_path.as_deref()).await?;

    let ssh_bind_addr = args
        .bind
        .or(config.system.bind)
        .unwrap_or(DEFAULT_BIND_ADDR);

    let metrics_bind_addr = args.metrics.or(config.system.metrics);

    let ssh_key_path = args.data.join("ssh.key");
    let ssh_key = keygen::init_from_path(&ssh_key_path).await?;

    let state = State::new(config);
    let state = Arc::new(ArcSwap::from_pointee(state));
    let (shared, rx) = Shared::new(state.clone());

    let sighup = signals::sighup(shared.tx.clone(), TaskEvent::ReloadConfig(config_path));

    let shared = Arc::new(shared);
    let mut server = ssh::SshServer::new(shared.clone());

    tokio::select! {
        res = metrics::start(metrics_bind_addr, shared) => res,
        res = state_machine(state, rx) => res,
        res = server.run(ssh_key.clone(), ssh_bind_addr) => res,
        res = sighup => Ok(res),
    }
}
