use clap::{ArgAction, Parser};
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version)]
pub struct Args {
    /// Increase logging output (can be used multiple times)
    #[arg(short, long, global = true, action(ArgAction::Count))]
    pub verbose: u8,
    /// Use a specific config file instead of auto-detect
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub subcommand: Subcommand,
}

#[derive(Debug, Clone, Parser)]
pub enum Subcommand {
    Ls(Ls),
    Inspect(Inspect),
    Keygen(Keygen),
    Hub(Hub),
    Agent(Agent),
    Status(Status),
    #[command(subcommand)]
    Plumbing(Plumbing),
}

/// Show known servers and patch status
#[derive(Debug, Clone, Parser)]
pub struct Ls;

/// Show detailed infos about a host
#[derive(Debug, Clone, Parser)]
pub struct Inspect;

/// Generate an ssh keypair
#[derive(Debug, Clone, Parser)]
pub struct Keygen {
    /// Read a private key on stdin and output it's public key
    #[arg(short = 'P', long, group = "action")]
    pub pubkey: bool,
    /// Read a public key on stdin and output it's fingerprint
    #[arg(short = 'F', long, group = "action")]
    pub fingerprint: bool,
}

/// Accept incoming agent connections
#[derive(Debug, Clone, Parser)]
pub struct Hub {
    /// The address to bind to
    pub bind: Option<SocketAddr>,
    /// The data directory to use
    #[arg(short = 'D', long, env = "PATCHUP_HUB_DATA")]
    pub data: PathBuf,
}

/// Report patch status to a hub
#[derive(Debug, Clone, Parser)]
pub struct Agent {
    /// The hub address to connect to
    pub addr: Option<SocketAddr>,
    /// The data directory to use
    #[arg(short = 'D', long, env = "PATCHUP_AGENT_DATA")]
    pub data: Option<PathBuf>,
    /// Connect as privileged process to socket to refresh patch status
    #[arg(short = 'R', long)]
    pub refresh: Option<PathBuf>,
}

/// Show status of this host's agent
#[derive(Debug, Clone, Parser)]
pub struct Status {
    // TODO: disabled for now, might reintroduce later
    /*
    /// Reload the host status first
    #[arg(short, long)]
    pub refresh: bool,
    */
    /// Output the status in JSON format
    #[arg(short, long)]
    pub json: bool,
    /// The agent socket to connect to
    #[arg(short = 'S', long, default_value = "/run/patchup.sock")]
    pub socket: PathBuf,
}

/// Internal plumbing commands
#[derive(Debug, Clone, Parser)]
pub enum Plumbing {
    CheckApk,
    CheckApt,
}
