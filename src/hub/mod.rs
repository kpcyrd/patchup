pub mod config;

use crate::args::Hub;
use crate::errors::*;
use std::path::Path;
use tokio::time::{self, Duration};

pub async fn run(_config: Option<&Path>, _args: &Hub) -> Result<()> {
    loop {
        info!("hub");
        time::sleep(Duration::from_secs(5)).await
    }
}
