use serde::{Deserialize, Serialize};
use serde_with::{DurationSeconds, serde_as};
use std::time::Duration;
use sysinfo::System;

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub hostname: String,
    pub os: String,
    pub os_id: String,
    pub arch: String,
    pub kernel: String,
    #[serde_as(as = "DurationSeconds<u64>")]
    pub uptime: Duration,
}

impl NodeInfo {
    pub fn query() -> Self {
        let os = System::name()
            .map(|os| {
                System::os_version()
                    .map(|ver| format!("{} {}", os, ver))
                    .unwrap_or(os)
            })
            .unwrap_or_else(|| "unknown".to_string());

        Self {
            hostname: System::host_name().unwrap_or_else(|| "-".to_string()),
            os,
            os_id: System::distribution_id(),
            arch: System::cpu_arch(),
            kernel: System::kernel_long_version(),
            uptime: Duration::from_secs(System::uptime()),
        }
    }
}
