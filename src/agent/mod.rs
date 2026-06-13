pub mod config;
pub mod patches;
pub mod refresh;
pub mod sandbox;
pub mod ssh;

use crate::agent::patches::UpdateStatus;
use crate::args::Agent;
use crate::errors::*;
use crate::ipc::{
    self,
    agent::{HubConnected, OfferRequest},
};
use crate::keygen;
use crate::node::NodeInfo;
use arc_swap::ArcSwap;
use russh::keys::PrivateKey;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use tokio::{
    fs,
    io::BufStream,
    net::{UnixListener, UnixStream},
    sync::{Notify, mpsc},
    task::JoinSet,
    time::{self, Duration},
};

pub const PATCH_REFRESH_INTERVAL: Duration = Duration::from_secs(60 * 60); // 1 hour
pub const OFFER_DEADLINE: Duration = Duration::from_secs(60 * 7); // 7 minutes
pub const HUB_PING_INTERVAL: Duration = Duration::from_secs(60 * 15); // 15 minutes

pub const HUB_PING_RETRY_INTERVAL: Duration = Duration::from_secs(45);
pub const HUB_PING_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
struct State {
    ssh_key: Arc<PrivateKey>,
    timers: Timers,
    hub: Option<ipc::agent::Hub>,
    updates: Option<BTreeMap<String, UpdateStatus>>,
}

impl State {
    fn new(ssh_key: PrivateKey) -> Self {
        Self {
            ssh_key: Arc::new(ssh_key),
            timers: Timers::default(),
            hub: None,
            updates: None,
        }
    }
}

#[derive(Debug, Clone)]
struct Timers {
    agent_uptime: time::Instant,
    last_refresh: Option<time::Instant>,
    last_refresh_offer: Option<time::Instant>,
}

impl Default for Timers {
    fn default() -> Self {
        Self {
            agent_uptime: time::Instant::now(),
            last_refresh: None,
            last_refresh_offer: None,
        }
    }
}

impl Timers {
    fn elapsed(&self) -> ipc::agent::Timers {
        let now = time::Instant::now();
        ipc::agent::Timers {
            agent_uptime: now.duration_since(self.agent_uptime),
            last_refresh: self.last_refresh.map(|t| now.duration_since(t)),
            last_refresh_offer: self.last_refresh_offer.map(|t| now.duration_since(t)),
        }
    }

    fn refresh_due(&self) -> bool {
        let Some(last) = self.last_refresh else {
            return true;
        };
        last.elapsed() >= PATCH_REFRESH_INTERVAL
    }
}

enum TaskEvent {
    RefreshOffered,
    SetUpdates(BTreeMap<String, UpdateStatus>),
    SetHub(ipc::agent::Hub),
}

async fn connector_task(
    state: &Arc<ArcSwap<State>>,
    notify: &Notify,
    _tx: &mpsc::Sender<TaskEvent>,
) -> Result<()> {
    // The previous state should include more info, including sysinfo, and hub configuration
    let mut last_state = (state.load().hub.clone(), state.load().updates.clone());
    let mut interval = time::interval(HUB_PING_INTERVAL);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                debug!("Timer for hub ping ticked");
            }
            _ = notify.notified() => {
                debug!("Received notify from state machine to inspect state and possibly notify hub");
                let state = state.load();
                debug!("state={:?}", state);

                // TODO: this check works but is very inefficient
                if last_state != (state.hub.clone(), state.updates.clone()) {
                    // Our internal state has changed, so notify the hub
                    info!("State changed, we should notify hub");
                    last_state = (state.hub.clone(), state.updates.clone());
                } else {
                    continue;
                }
            }
        };

        // TODO: also notify hub when timers are overdue
        // TODO: also notify hub when hub config has changed

        let state = state.load();
        debug!("state={:?}", state);

        let Some(hub) = &state.hub else {
            debug!("No hub configured, skipping notification");
            continue;
        };

        match time::timeout(
            HUB_PING_TIMEOUT,
            ssh::submit_to_hub(
                hub.addr,
                state.ssh_key.clone(),
                hub.server_key.clone(),
                &state.updates,
            ),
        )
        .await
        {
            Ok(Ok(())) => {
                info!("Successfully notified hub");

                last_state = (state.hub.clone(), state.updates.clone());
                // Reset the interval timer, in case this was due to a state change and not a regular tick
                interval.reset();
            }
            Ok(Err(err)) => {
                error!("Failed to notify hub: {err:?}");
                interval.reset_after(HUB_PING_RETRY_INTERVAL);
            }
            Err(err) => {
                error!("Timed out while trying to notify hub: {err:?}");
                interval.reset_after(HUB_PING_RETRY_INTERVAL);
            }
        }
    }
}

// Only this task is allowed to update the state
async fn state_machine(
    state: &Arc<ArcSwap<State>>,
    notify: &Notify,
    mut rx: mpsc::Receiver<TaskEvent>,
) -> Result<()> {
    loop {
        let Some(msg) = rx.recv().await else {
            break Ok(());
        };

        match msg {
            TaskEvent::RefreshOffered => {
                debug!("Updating refresh offer timer");
                let mut new = state.load().as_ref().clone();
                new.timers.last_refresh_offer = Some(time::Instant::now());
                state.store(Arc::new(new));
            }
            TaskEvent::SetUpdates(updates) => {
                debug!("Updating package manager update status");
                let mut new = state.load().as_ref().clone();
                new.updates = Some(updates);
                new.timers.last_refresh = Some(time::Instant::now());
                state.store(Arc::new(new));

                notify.notify_one();
            }
            TaskEvent::SetHub(hub) => {
                debug!("Updating hub configuration");
                let mut new = state.load().as_ref().clone();
                new.hub = Some(hub);
                state.store(Arc::new(new));

                notify.notify_one();
            }
        }
    }
}

async fn ipc_server(
    state: &Arc<ArcSwap<State>>,
    socket: UnixListener,
    tx: &mpsc::Sender<TaskEvent>,
) -> Result<()> {
    let mut set = JoinSet::new();
    loop {
        tokio::select! {
            Some(Ok(res)) = set.join_next() => {
                match res {
                    Ok(()) => debug!("IPC socket client disconnected: {res:?}"),
                    Err(err) => warn!("IPC socket client error: {err:?}"),
                }
            }
            res = socket.accept() => {
                let (stream, _addr) = res?;
                info!("Accepted unix socket connection");
                let state = state.clone();
                let tx = tx.clone();
                set.spawn(async move {
                    serve_socket_client(state, stream, tx).await
                });
            }
        }
    }
}

async fn serve_socket_client(
    state: Arc<ArcSwap<State>>,
    stream: UnixStream,
    tx: mpsc::Sender<TaskEvent>,
) -> Result<()> {
    let mut stream = BufStream::new(stream);

    loop {
        // TODO: this needs an allocation limit
        let Some(req) = ipc::recv_opt::<_, ipc::agent::Request>(&mut stream).await? else {
            break;
        };

        match req {
            ipc::agent::Request::Status => {
                let state = state.load();
                ipc::send(
                    &mut stream,
                    &ipc::agent::Status {
                        ssh_key: state.ssh_key.public_key().clone(),
                        node: NodeInfo::query(),
                        timers: state.timers.elapsed(),
                        updates: state.updates.clone(),
                    },
                )
                .await?;
            }
            ipc::agent::Request::Refresh { mandatory } => {
                info!("Received refresh offer (mandatory={mandatory})");
                tx.send(TaskEvent::RefreshOffered).await?;

                if mandatory || state.load().timers.refresh_due() {
                    debug!("Accepting refresh offer");

                    ipc::send(&mut stream, &OfferRequest::ListPkgBackends).await?;
                    let backends = ipc::recv::<_, Vec<String>>(&mut stream).await?;
                    info!("Received pkg backends: {backends:?}");

                    let mut updates = BTreeMap::new();

                    for backend in &backends {
                        debug!("Querying pkg backend: {backend}");
                        ipc::send(
                            &mut stream,
                            &OfferRequest::QueryPkgBackend {
                                name: backend.clone(),
                            },
                        )
                        .await?;
                        let status = ipc::recv::<_, UpdateStatus>(&mut stream).await?;
                        info!("Received status for {backend}: {status:?}");
                        updates.insert(backend.clone(), status);
                    }

                    debug!("Finished querying pkg backends");
                    tx.send(TaskEvent::SetUpdates(updates)).await?;
                } else {
                    debug!("Declining refresh offer, not due yet");
                }

                // We are done, disconnect the remote process
                break;
            }
            ipc::agent::Request::ConnectHub { hub } => {
                let state = state.load();

                let error = match time::timeout(
                    HUB_PING_TIMEOUT,
                    ssh::connect(
                        hub.addr,
                        ssh::AGENT_USER,
                        state.ssh_key.clone(),
                        hub.server_key.clone(),
                    ),
                )
                .await
                {
                    Ok(Ok(ssh)) => {
                        info!("Successfully connected to hub");
                        drop(ssh);

                        // Store the hub configuration in the state and file
                        tx.send(TaskEvent::SetHub(hub)).await?;

                        None
                    }
                    Ok(Err(err)) => Some(format!("Failed to connect to hub: {err:?}")),
                    Err(err) => Some(format!("Failed to connect to hub: {err:?}")),
                };

                ipc::send(&mut stream, &HubConnected { error }).await?;
            }
        }
    }

    Ok(())
}

async fn bind() -> Result<UnixListener> {
    let socket = if let Ok(fds) = sd_listen_fds::get()
        && let num_fds = fds.len()
        && let Some((name, fd)) = fds.into_iter().next()
    {
        info!(
            "Received {} sockets from systemd, using first one: {}",
            num_fds,
            name.map(|n| format!("{n:?}")).as_deref().unwrap_or("-")
        );
        let fd = fd.into_std();
        let fd = std::os::unix::net::UnixListener::from(fd);
        fd.set_nonblocking(true)
            .context("Failed to set socket non-blocking")?;
        UnixListener::from_std(fd).context("Failed to use sd-listen socket from systemd")?
    } else {
        // TODO: use proper path
        let socket_path = "data/agent/patchup-agent.sock";
        fs::remove_file(socket_path).await.ok();
        debug!("Binding to socket: {socket_path:?}");
        UnixListener::bind(socket_path)
            .with_context(|| format!("Failed to bind socket: {socket_path:?}"))?
    };
    Ok(socket)
}

pub async fn run(_config: Option<&Path>, args: &Agent) -> Result<()> {
    let data_dir = args
        .data
        .as_ref()
        .context("Data directory is required to start agent")?;

    fs::create_dir_all(data_dir)
        .await
        .with_context(|| format!("Failed to create directory: {data_dir:?}"))?;

    let socket = bind().await?;
    sandbox::init();

    let ssh_key_path = data_dir.join("ssh.key");
    let ssh_key = keygen::init_from_path(&ssh_key_path).await?;

    let state = Arc::new(ArcSwap::from_pointee(State::new(ssh_key)));
    let (tx, rx) = mpsc::channel(5);
    let notify = Notify::new();

    tokio::select! {
        res = connector_task(&state, &notify, &tx) => res,
        res = state_machine(&state, &notify, rx) => res,
        res = ipc_server(&state, socket, &tx) => res,
    }
}
