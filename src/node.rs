use crate::agent::patches::UpdateStatus;
use platform_info::{PlatformInfo, PlatformInfoAPI, UNameAPI};
use serde::{Deserialize, Serialize};
// use serde_with::{DurationSeconds, serde_as};
use std::collections::BTreeMap;

const PATCHUP_VERSION: &str = env!("CARGO_PKG_VERSION");

// #[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeInfo {
    pub patchup_version: String,
    pub hostname: String,
    pub os: String,
    pub os_id: String,
    pub arch: String,
    pub kernel: String,
    pub updates: Option<BTreeMap<String, UpdateStatus>>,
    pub pending_kernel: Option<String>,
    /*
    #[serde_as(as = "DurationSeconds<u64>")]
    pub uptime: Duration,
    */
}

fn hostname(platform: Option<&PlatformInfo>) -> String {
    let Some(p) = platform else {
        return "-".to_string();
    };

    p.nodename().to_string_lossy().into_owned()
}

fn os(platform: Option<&PlatformInfo>) -> (String, String) {
    #[cfg(target_os = "openbsd")]
    return {
        let os = if let Some(p) = platform {
            let release = p.release().to_string_lossy();
            format!("OpenBSD {release}")
        } else {
            "OpenBSD".to_string()
        };

        let os_id = "openbsd".to_string();

        (os, os_id)
    };

    #[cfg(not(target_os = "openbsd"))]
    return {
        // This value is not used on this platform
        let _ = platform;

        let os = sysinfo::System::name()
            .map(|os| {
                sysinfo::System::os_version()
                    .map(|ver| format!("{} {}", os, ver))
                    .unwrap_or(os)
            })
            .unwrap_or_else(|| "unknown".to_string());

        let os_id = sysinfo::System::distribution_id();

        (os, os_id)
    };
}

fn arch(platform: Option<&PlatformInfo>) -> String {
    let Some(p) = platform else {
        return "-".to_string();
    };

    p.processor().to_string_lossy().into_owned()
}

fn kernel(platform: Option<&PlatformInfo>) -> String {
    let Some(p) = platform else {
        return "-".to_string();
    };

    let sys = p.sysname().to_string_lossy();
    let release = p.release().to_string_lossy();

    if cfg!(target_os = "openbsd") {
        let build = p.version().to_string_lossy();
        format!("{sys} {release} {build}")
    } else {
        format!("{sys} {release}")
    }
}

impl NodeInfo {
    pub fn query() -> Self {
        let platform = PlatformInfo::new().ok();

        let (os, os_id) = os(platform.as_ref());

        Self {
            patchup_version: PATCHUP_VERSION.to_string(),
            hostname: hostname(platform.as_ref()),
            os,
            os_id,
            arch: arch(platform.as_ref()),
            kernel: kernel(platform.as_ref()),
            updates: None,
            pending_kernel: None,
            // uptime: Duration::from_secs(sysinfo::System::uptime()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux() {
        let nodeinfo = NodeInfo::query();
        assert!(nodeinfo.kernel.starts_with("Linux "));
    }

    #[cfg(target_os = "openbsd")]
    #[test]
    fn test_linux() {
        let nodeinfo = NodeInfo::query();
        assert!(nodeinfo.kernel.starts_with("OpenBSD "));
    }
}
