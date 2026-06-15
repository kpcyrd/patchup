use crate::errors::*;
use std::future;
use tokio::sync::mpsc;

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
