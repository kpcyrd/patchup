use clap::{ArgAction, Parser, ValueHint};
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version)]
pub struct Args {
    /// Increase logging output (can be used multiple times)
    #[arg(short, long, global = true, action(ArgAction::Count))]
    pub verbose: u8,
    /// Silent output (except errors)
    #[arg(short, long, global = true)]
    pub quiet: bool,
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
    Connect(Connect),
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
    #[arg(short = 'B', long)]
    pub bind: Option<SocketAddr>,
    /// The data directory to use
    #[arg(short = 'D', long, env = "PATCHUP_HUB_DATA", value_name = "PATH", value_hint = ValueHint::DirPath)]
    pub data: PathBuf,
    /// Bind a port for http prometheus metrics
    #[arg(long)]
    pub metrics: Option<SocketAddr>,
}

/// Report patch status to a hub
#[derive(Debug, Clone, Parser)]
pub struct Agent {
    /// The hub address to connect to
    pub addr: Option<SocketAddr>,
    /// The data directory to use
    #[arg(short = 'D', long, env = "PATCHUP_AGENT_DATA", value_name = "PATH", value_hint = ValueHint::DirPath)]
    pub data: Option<PathBuf>,
    /// Connect as privileged process to socket to refresh patch status
    #[arg(short = 'R', long, value_name = "PATH", value_hint = ValueHint::FilePath)]
    pub refresh: Option<PathBuf>,
    #[command(flatten)]
    pub socket: Socket,
}

/// Configure a hub for an agent
#[derive(Debug, Clone, Parser)]
pub struct Connect {
    /// The hub address to connect to
    pub addr: Option<SocketAddr>,
    /// Automatically answer yes (once to use existing hub config, twice to accept any server key)
    #[arg(short, long, action(ArgAction::Count))]
    pub yes: u8,
    #[command(flatten)]
    pub socket: Socket,
    //
    // TODO: It should be possible to configure non-iteractively
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
    #[command(flatten)]
    pub socket: Socket,
    #[command(flatten)]
    pub output: Output,
}

/// Internal plumbing commands
#[derive(Debug, Clone, Parser)]
pub enum Plumbing {
    CheckApk {
        #[command(flatten)]
        output: Output,
    },
    CheckApt {
        #[command(flatten)]
        output: Output,
    },
    CheckPacman {
        #[command(flatten)]
        output: Output,
    },
    ScanLinuxKernels,
}

#[derive(Debug, Clone, Parser)]
pub struct Socket {
    /// The agent socket path to use
    #[arg(short = 'S', long = "socket", default_value = "/run/patchup/socket", value_hint = ValueHint::FilePath)]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Parser)]
pub struct Output {
    /// Output the status in JSON format
    #[arg(short, long)]
    pub json: bool,
}
