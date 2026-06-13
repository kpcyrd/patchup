pub mod config;
mod metrics;
mod ssh;

use crate::args::Hub;
use crate::errors::*;
use crate::keygen;
use arc_swap::ArcSwap;
use russh::keys::PublicKey;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;

const CHANNEL_BACKLOG: usize = 64;

#[derive(Debug, Clone)]
struct State {
    nodes: BTreeMap<PublicKey, ()>,
}

impl State {
    pub fn new() -> Self {
        Self {
            nodes: Default::default(),
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

    pub async fn ping_from_node(&self, public_key: PublicKey) -> Result<()> {
        self.tx
            .send(TaskEvent::PingNode(public_key))
            .await
            .context("Failed to record ping from node")
    }
}

#[derive(Debug)]
enum TaskEvent {
    PingNode(PublicKey),
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
            TaskEvent::PingNode(public_key) => {
                debug!("Ping from node: {public_key:?}");
                let mut new = state.load().as_ref().clone();
                new.nodes.insert(public_key, ());
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
