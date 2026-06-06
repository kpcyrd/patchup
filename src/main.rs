pub mod agent;
pub mod args;
pub mod config;
pub mod errors;
pub mod hub;
pub mod ipc;
pub mod keygen;
pub mod node;
pub mod wire;

use crate::args::{Args, Plumbing, Subcommand};
use crate::config::Config;
use crate::errors::*;
use clap::Parser;
use colored::Colorize;
use env_logger::Env;
// use etcetera::BaseStrategy;
use russh::keys::{HashAlg, PrivateKey, PublicKey};
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = match args.verbose {
        0 => "info",
        1 => "info,patchup=debug",
        2 => "debug",
        3 => "debug,patchup=trace",
        _ => "trace",
    };
    env_logger::Builder::from_env(Env::default().default_filter_or(log_level)).init();

    debug!("args: {args:?}");

    /*
    let strategy = etcetera::choose_base_strategy().unwrap();

    debug!("config-dir: {:?}", strategy.config_dir());
    debug!("data-dir: {:?}", strategy.data_dir());
    */

    match &args.subcommand {
        Subcommand::Ls(_ls) => {
            let config = Config::load(args.config.as_deref()).await?;
            info!("config: {config:?}");
            // TODO:
        }
        Subcommand::Inspect(_inspect) => {
            let config = Config::load(args.config.as_deref()).await?;
            info!("config: {config:?}");
            // TODO:
        }
        Subcommand::Keygen(keygen) => {
            if keygen.pubkey || keygen.fingerprint {
                let mut stdin = tokio::io::stdin();
                let mut buf = Vec::new();
                stdin.read_to_end(&mut buf).await?;

                if keygen.pubkey {
                    let privkey = PrivateKey::from_openssh(&buf)?;
                    let pubkey = privkey.public_key();
                    let key = pubkey.to_openssh()?;
                    println!("{}", key.as_str());
                } else {
                    let buf = str::from_utf8(&buf)?;
                    let pubkey = PublicKey::from_openssh(buf)?;
                    let fp = pubkey.fingerprint(HashAlg::Sha256);
                    println!("{}", fp);
                }
            } else {
                let key = keygen::keygen_str()?;
                println!("{}", key.as_str().trim());
            }
        }
        Subcommand::Hub(hub) => {
            hub::run(args.config.as_deref(), hub).await?;
        }
        Subcommand::Agent(agent) => {
            // This is within the same subcommand because it's the privileged component of the agent
            if let Some(path) = &agent.refresh {
                let mandatory = false;
                agent::refresh::offer(path, mandatory).await?;
            } else {
                agent::run(args.config.as_deref(), agent).await?;
            }
        }
        Subcommand::Status(args) => {
            let mut sock = ipc::agent::AgentIpc::connect("data/agent/patchup-agent.sock").await?;
            let status = sock.status().await?;
            if args.json {
                let json = serde_json::to_string_pretty(&status)?;
                println!("{json}");
            } else {
                println!("{}  {}", "hostname: ".bold(), status.node.hostname);
                println!("{}  {}", "os:       ".bold(), status.node.os);
                println!("{}  {}", "arch:     ".bold(), status.node.arch);
                println!("{}  {}", "kernel:   ".bold(), status.node.kernel);
                println!(
                    "{}  {}",
                    "uptime:   ".bold(),
                    humantime::format_duration(status.node.uptime)
                );
            }
        }
        Subcommand::Plumbing(plumbing) => match plumbing {
            Plumbing::CheckApk => {
                agent::patches::apk::run().await?;
            }
        },
    }

    Ok(())
}
