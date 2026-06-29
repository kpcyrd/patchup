pub mod agent;
pub mod args;
pub mod config;
pub mod errors;
pub mod hub;
pub mod ipc;
pub mod keygen;
pub mod node;
pub mod prompt;
pub mod signals;
pub mod ssh;
pub mod wire;

use crate::agent::ssh::ServerKeyVerification;
use crate::args::{Args, Plumbing, Subcommand};
use crate::config::Config;
use crate::errors::*;
use crate::prompt::Prompt;
use clap::Parser;
use colored::Colorize;
use env_logger::Env;
use std::net::SocketAddr;
use tokio::sync::mpsc;
// use etcetera::BaseStrategy;
use russh::keys::{HashAlg, PrivateKey, PublicKey};
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = match (args.quiet, args.verbose) {
        (true, _) => "error",
        (_, 0) => "info",
        (_, 1) => "info,patchup=debug",
        (_, 2) => "debug",
        (_, 3) => "debug,patchup=trace",
        (_, _) => "trace",
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
            hub::run(args.config, hub).await?;
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
        Subcommand::Connect(args) => {
            let mut sock = ipc::agent::AgentIpc::connect(&args.socket.path).await?;
            let status = sock.status().await?;

            let mut prompt = Prompt::new();

            // Show the agent ssh key so it can be added to the hub configuration
            println!("{} {}", "ssh key:   ".bold(), status.ssh_key.to_openssh()?);
            println!();

            // If we are already connected, ask if we just want to do an explicit ping
            if let Some(hub) = &status.hub {
                println!("{} {}", "hub address:    ".bold(), hub.addr);
                println!(
                    "{} {}",
                    "server ssh key: ".bold(),
                    hub.server_key.to_openssh()?
                );
                println!();

                let use_existing = if args.yes == 1 {
                    true
                } else {
                    let yes_no = prompt
                        .get::<prompt::YesNo>("use existing hub config? [yes/no]: ")
                        .await?;
                    println!();
                    yes_no.is_yes()
                };

                if use_existing {
                    sock.ping_hub().await?;
                    println!("requested agent to ping hub");
                    return Ok(());
                }
            }

            // Ask for the hub address if not provided as argument
            let hub_addr = if let Some(addr) = args.addr {
                println!("hub address [ip:port]: {}", addr);
                addr
            } else {
                prompt.get::<SocketAddr>("hub address [ip:port]: ").await?
            };
            println!();

            // Connect to the hub and get their ssh public key
            let (tx, mut rx) = mpsc::channel(1);

            let ssh = agent::ssh::connect_anonymous(
                hub_addr,
                "patchup",
                ServerKeyVerification::Report(tx),
            )
            .await?;
            drop(ssh);
            let server_key = rx.try_recv()?;

            println!(
                "{} {}",
                "server ssh key:  ".bold(),
                server_key.to_openssh()?
            );
            println!(
                "{} {}",
                "sha256:          ".bold(),
                server_key.fingerprint(HashAlg::Sha256)
            );
            println!();

            // Check if it's already known, otherwise ask if we want to accept it
            let accept_key = if args.yes == 2 {
                true
            } else {
                let yes_no = prompt
                    .get::<prompt::YesNo>("accept key? [yes/no]: ")
                    .await?;
                println!();
                yes_no.is_yes()
            };

            if !accept_key {
                println!("setup canceled by user");
                return Ok(());
            }

            // Check if we can authenticate and the server speaks our protocol
            // If so, persist the configuration in the agent
            sock.connect_hub(ipc::agent::Hub {
                addr: hub_addr,
                server_key,
            })
            .await?;
            println!("successfully connected to hub and authenticated with ssh key");
        }
        Subcommand::Status(args) => {
            let mut sock = ipc::agent::AgentIpc::connect(&args.socket.path).await?;
            let status = sock.status().await?;
            if args.output.json {
                let json = serde_json::to_string_pretty(&status)?;
                println!("{json}");
            } else {
                println!("{}  {}", "hostname: ".bold(), status.node.hostname);
                println!("{}  {}", "os:       ".bold(), status.node.os);
                println!("{}  {}", "arch:     ".bold(), status.node.arch);
                println!("{}  {}", "kernel:   ".bold(), status.node.kernel);
                /*
                println!(
                    "{}  {}",
                    "uptime:   ".bold(),
                    humantime::format_duration(status.node.uptime)
                );
                println!();
                */

                println!("{} {}", "ssh key:   ".bold(), status.ssh_key.to_openssh()?);
                println!(
                    "{} {}",
                    "sha256:    ".bold(),
                    status.ssh_key.fingerprint(HashAlg::Sha256)
                );

                println!();
                println!("{}", "updates:".bold());
                if let Some(updates) = status.node.updates {
                    if !updates.is_empty() {
                        for (ecosystem, status) in updates {
                            let ecosystem = format!("{ecosystem}: ");

                            let (num, nomen) = match status.pending.len() {
                                0 => ("0".green(), "updates"),
                                1 => ("1".yellow(), "update"),
                                n => (n.to_string().yellow(), "updates"),
                            };
                            let hint = if status.refresh_error {
                                " (failed to refresh)".red().bold()
                            } else {
                                Default::default()
                            };
                            println!(
                                "  {:<8}  {} pending {}{}",
                                ecosystem.bold(),
                                num.bold(),
                                nomen,
                                hint
                            );

                            for update in status.pending {
                                println!("            - {}", update.name);
                            }
                        }
                    } else {
                        println!("  {}", "No package manager detected".italic());
                    }
                } else {
                    println!("  {}", "Waiting for privileged process".italic());
                }

                if status.timers.refresh_offer_overdue() {
                    println!();
                    println!("  {}", "Periodic privileged process is overdue, cronjob/timer may not be configured correctly".red().bold());
                }
            }
        }
        Subcommand::Plumbing(plumbing) => match plumbing {
            Plumbing::CheckApk { output } => {
                agent::patches::apk::run(output).await?;
            }
            Plumbing::CheckApt { output } => {
                agent::patches::apt::run(output).await?;
            }
            Plumbing::CheckPacman { output } => {
                agent::patches::pacman::run(output).await?;
            }
            Plumbing::ScanLinuxKernels => {
                let kernels = agent::kernels::linux::list_available().await?;
                if let Some(max) = kernels.iter().max() {
                    info!("Latest kernel version: {:?}", max.as_str());
                }
            }
        },
    }

    Ok(())
}
