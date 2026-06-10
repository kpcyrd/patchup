use crate::agent::patches::{Update, UpdateStatus};
use crate::args::Output;
use crate::errors::*;
use tokio::fs;
use tokio::io::ErrorKind;

pub const ID: &str = "apk";
const PATH: &str = "/lib/apk/db/installed";

fn parse(data: &str) -> Vec<Update> {
    data.lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(|s| Update {
            name: s.to_string(),
        })
        .collect()
}

pub async fn detect() -> bool {
    fs::metadata(PATH)
        .await
        .err()
        .filter(|err| err.kind() == ErrorKind::NotFound)
        .is_none()
}

pub async fn query() -> Result<UpdateStatus> {
    let mut status = UpdateStatus::default();

    debug!("Running apk update");
    let update = tokio::process::Command::new("apk")
        .arg("update")
        .output()
        .await
        .context("Failed to run apk update")?;

    if !update.status.success() {
        warn!(
            "apk update failed: {:?}",
            String::from_utf8_lossy(&update.stderr)
        );
        status.refresh_error = true;
    }

    debug!("Running apk list --upgradable");
    let list = tokio::process::Command::new("apk")
        .args(["list", "--upgradable"])
        .output()
        .await
        .context("Failed to run apk list")?;
    if !list.status.success() {
        bail!(
            "apk list failed: {:?}",
            String::from_utf8_lossy(&list.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&list.stdout);
    status.pending = parse(&stdout);

    Ok(status)
}

pub async fn run(output: &Output) -> Result<()> {
    if !detect().await {
        warn!("apk database not found, skipping");
        return Ok(());
    }

    let status = query().await?;

    if output.json {
        let json = serde_json::to_string_pretty(&status)?;
        println!("{json}");
    } else {
        for update in status.pending {
            info!("Update available: {update:?}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let data = "alpine-baselayout-3.7.2-r0 x86_64 {alpine-baselayout} (GPL-2.0-only) [upgradable from: alpine-baselayout-3.7.1-r8]
alpine-baselayout-data-3.7.2-r0 x86_64 {alpine-baselayout} (GPL-2.0-only) [upgradable from: alpine-baselayout-data-3.7.1-r8]
alpine-release-3.23.4-r0 x86_64 {alpine-base} (MIT) [upgradable from: alpine-release-3.23.3-r0]
apk-tools-3.0.6-r0 x86_64 {apk-tools} (GPL-2.0-only) [upgradable from: apk-tools-3.0.3-r1]
ca-certificates-20260413-r0 x86_64 {ca-certificates} (MPL-2.0 AND MIT) [upgradable from: ca-certificates-20251003-r0]
ca-certificates-bundle-20260413-r0 x86_64 {ca-certificates} (MPL-2.0 AND MIT) [upgradable from: ca-certificates-bundle-20251003-r0]
libapk-3.0.6-r0 x86_64 {apk-tools} (GPL-2.0-only) [upgradable from: libapk-3.0.3-r1]
libcrypto3-3.5.6-r0 x86_64 {openssl} (Apache-2.0) [upgradable from: libcrypto3-3.5.5-r0]
libssl3-3.5.6-r0 x86_64 {openssl} (Apache-2.0) [upgradable from: libssl3-3.5.5-r0]
musl-1.2.5-r23 x86_64 {musl} (MIT) [upgradable from: musl-1.2.5-r21]
musl-dev-1.2.5-r23 x86_64 {musl} (MIT) [upgradable from: musl-dev-1.2.5-r21]
musl-utils-1.2.5-r23 x86_64 {musl} (MIT AND BSD-2-Clause AND GPL-2.0-or-later) [upgradable from: musl-utils-1.2.5-r21]
zlib-1.3.2-r0 x86_64 {zlib} (Zlib) [upgradable from: zlib-1.3.1-r2]
";
        let updates = parse(data);
        assert_eq!(
            updates,
            vec![
                Update {
                    name: "alpine-baselayout-3.7.2-r0".to_string()
                },
                Update {
                    name: "alpine-baselayout-data-3.7.2-r0".to_string()
                },
                Update {
                    name: "alpine-release-3.23.4-r0".to_string()
                },
                Update {
                    name: "apk-tools-3.0.6-r0".to_string()
                },
                Update {
                    name: "ca-certificates-20260413-r0".to_string()
                },
                Update {
                    name: "ca-certificates-bundle-20260413-r0".to_string()
                },
                Update {
                    name: "libapk-3.0.6-r0".to_string()
                },
                Update {
                    name: "libcrypto3-3.5.6-r0".to_string()
                },
                Update {
                    name: "libssl3-3.5.6-r0".to_string()
                },
                Update {
                    name: "musl-1.2.5-r23".to_string()
                },
                Update {
                    name: "musl-dev-1.2.5-r23".to_string()
                },
                Update {
                    name: "musl-utils-1.2.5-r23".to_string()
                },
                Update {
                    name: "zlib-1.3.2-r0".to_string()
                },
            ]
        );
    }

    #[test]
    fn test_parse_empty() {
        let updates = parse("");
        assert_eq!(updates, &[]);
    }
}
