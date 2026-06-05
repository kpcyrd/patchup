pub mod config;
pub mod patches;
pub mod refresh;

use crate::args::Agent;
use crate::errors::*;
use crate::ipc;
use crate::node::NodeInfo;
use arc_swap::ArcSwap;
use std::path::Path;
use std::sync::Arc;
use tokio::{
    fs,
    io::BufStream,
    net::{UnixListener, UnixStream},
    task::JoinSet,
    time::{self, Duration},
};

#[derive(Debug, Default)]
struct State {
    // TODO
}

async fn connect(_state: &ArcSwap<State>) -> Result<()> {
    loop {
        debug!("connect");
        time::sleep(Duration::from_secs(5)).await
    }
}

async fn query(_state: &ArcSwap<State>) -> Result<()> {
    loop {
        debug!("query");
        time::sleep(Duration::from_secs(5)).await
    }
}

async fn status_socket(state: &Arc<ArcSwap<State>>, socket: UnixListener) -> Result<()> {
    let mut set = JoinSet::new();
    loop {
        tokio::select! {
            Some(Ok(res)) = set.join_next() => {
                match res {
                    Ok(()) => debug!("Status socket client disconnected: {res:?}"),
                    Err(err) => warn!("Status socket client error: {err:?}"),
                }
            }
            res = socket.accept() => {
                let (stream, _addr) = res?;
                info!("Accepted unix socket connection");
                let state = state.clone();
                set.spawn(async move {
                    serve_socket_client(state, stream).await
                });
            }
        }
    }
}

async fn serve_socket_client(_state: Arc<ArcSwap<State>>, stream: UnixStream) -> Result<()> {
    let mut stream = BufStream::new(stream);

    loop {
        // TODO: this needs an allocation limit
        let Some(req) = ipc::recv_opt::<_, ipc::agent::Request>(&mut stream).await? else {
            break;
        };

        match req {
            ipc::agent::Request::Status => {
                ipc::send(
                    &mut stream,
                    &ipc::agent::Status {
                        node: NodeInfo::query(),
                    },
                )
                .await?;
            }
            ipc::agent::Request::Refresh { mandatory } => {
                info!("Received refresh offer (mandatory={mandatory})");
                // TODO: This is currently not implemented yet
                ipc::send(&mut stream, &()).await?;
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
            "Received {} sockets from systemd, using first one: {name:?}",
            num_fds
        );
        let fd = fd.into_std();
        let fd = std::os::unix::net::UnixListener::from(fd);
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

pub async fn run(_config: Option<&Path>, _args: &Agent) -> Result<()> {
    let socket = bind().await?;
    let state = Arc::new(ArcSwap::from_pointee(State::default()));

    tokio::select! {
        res = connect(&state) => res,
        res = query(&state) => res,
        res = status_socket(&state, socket) => res,
    }
}
