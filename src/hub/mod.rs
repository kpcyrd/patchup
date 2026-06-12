pub mod config;
mod metrics;
mod ssh;

use crate::args::Hub;
use crate::errors::*;
use crate::keygen;
use std::path::Path;

pub async fn run(_config: Option<&Path>, args: &Hub) -> Result<()> {
    let mut server = ssh::SshServer::new();

    let ssh_key_path = args.data.join("ssh.key");
    let ssh_key = keygen::init_from_path(&ssh_key_path).await?;

    tokio::select! {
        res = metrics::start(args.metrics) => res,
        res = server.run(ssh_key.clone(), args.bind) => res,
    }
}
