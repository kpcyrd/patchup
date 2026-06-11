pub mod config;
mod ssh;

use crate::args::Hub;
use crate::errors::*;
use crate::keygen;
use std::path::Path;
use tokio::time::{self, Duration};

pub async fn run(_config: Option<&Path>, args: &Hub) -> Result<()> {
    let mut server = ssh::SshServer::new();

    let ssh_key_path = args.data.join("ssh.key");
    let ssh_key = keygen::init_from_path(&ssh_key_path).await?;

    server.run(ssh_key, args.bind).await?;

    loop {
        info!("hub");
        time::sleep(Duration::from_secs(5)).await
    }
}
