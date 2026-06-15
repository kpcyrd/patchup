use crate::errors::*;
use std::future;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

// Handle shutdown signals so we can run this as pid1
pub async fn sigterm() {
    let mut set = JoinSet::new();

    // On ctrl-c, shutdown
    set.spawn(async {
        let _ = tokio::signal::ctrl_c().await;
    });

    #[cfg(unix)]
    {
        // On SIGTERM, shutdown
        use tokio::signal::unix;
        if let Ok(mut signal) = unix::signal(unix::SignalKind::terminate()) {
            set.spawn(async move {
                signal.recv().await;
            });
        }
    }

    set.join_next().await;
}

pub async fn sighup<T: Clone>(tx: mpsc::Sender<T>, msg: T) {
    #[cfg(unix)]
    {
        use tokio::signal::unix;
        if let Ok(mut signals) = unix::signal(unix::SignalKind::hangup()) {
            while signals.recv().await.is_some() {
                info!("Received SIGHUP, reloading configuration");
                if let Err(e) = tx.send(msg.clone()).await {
                    error!("Failed to send SIGHUP message: {:?}", e);
                }
            }
        }
    }

    // Reload signals not supported, wait indefinitely
    future::pending().await
}
