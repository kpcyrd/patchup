use serde::{Deserialize, Serialize};
// use std::collections::BTreeMap;
// use std::time::Duration;

use crate::node::NodeInfo;

#[derive(Debug, Serialize, Deserialize)]
pub struct Hello {
    pub info: NodeInfo,
}

/*
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub kernel: Option<String>,
    pub uptime: Duration,
    pub updates: BTreeMap<String, Updates>,
}
*/

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Updates {
    pub pending: Vec<String>,
    pub security: Vec<String>,
    pub stale_code: Vec<Process>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Process {
    pub pid: u64, // TODO: check the size of PIDs
    pub cmdline: Vec<String>,
    pub uid: u64, // TODO: check the size
}
