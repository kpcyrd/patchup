use crate::agent::patches::{Update, UpdateStatus};
use crate::args::Output;
use crate::errors::*;
use async_tempfile::TempDir;
use std::ffi::OsStr;
use tokio::fs;
use tokio::io::ErrorKind;

pub const ID: &str = "pacman";
const PATH: &str = "/var/lib/pacman/local";

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

    let dir = TempDir::builder()
        .prefix("patchup-pacman_")
        .create()
        .await?;
    debug!("Taking pacman database snapshot: {:?}", dir.dir_path());

    if let Err(err) = fs::symlink("/var/lib/pacman/local", dir.join("local")).await {
        dir.drop_async().await;
        return Err(err).context("Failed to create symlink for pacman database");
    }

    debug!("Running pacman -Sy on database snapshot");
    let update = tokio::process::Command::new("fakeroot")
        .args([
            OsStr::new("pacman"),
            OsStr::new("-Sy"),
            OsStr::new("--disable-sandbox-filesystem"),
            OsStr::new("--dbpath"),
            dir.dir_path().as_os_str(),
            OsStr::new("--logfile"),
            OsStr::new("/dev/null"),
        ])
        .output()
        .await
        .context("Failed to run fakeroot pacman -Sy")?;
    if !update.status.success() {
        warn!(
            "fakeroot pacman -Sy failed: {:?}",
            String::from_utf8_lossy(&update.stderr)
        );
        status.refresh_error = true;
    }

    debug!("Running pacman -Qu");
    let list = tokio::process::Command::new("pacman")
        .args([
            OsStr::new("-Qu"),
            OsStr::new("--dbpath"),
            dir.dir_path().as_os_str(),
        ])
        .output()
        .await
        .context("Failed to run pacman -Qu")?;
    if !list.status.success() && !list.stderr.is_empty() {
        dir.drop_async().await;

        bail!(
            "pacman -Qu failed: {:?}",
            String::from_utf8_lossy(&list.stderr)
        );
    }

    debug!("Cleaning up pacman database snapshot");
    dir.drop_async().await;

    // Parsing output
    let stdout = String::from_utf8_lossy(&list.stdout);
    status.pending = parse(&stdout);

    Ok(status)
}

pub async fn run(output: &Output) -> Result<()> {
    if !detect().await {
        warn!("pacman database not found, skipping");
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

    #[tokio::test]
    async fn test_parse() {
        let data = "libnvme 1.16.1-3 -> 1.16.2-1
lighttpd 1.4.82-2 -> 1.4.84-1
linux-api-headers 7.0-1 -> 7.1-1
linux-hardened 7.0.11.hardened2-1 -> 7.0.12.hardened1-1
linux-hardened-docs 7.0.11.hardened2-1 -> 7.0.12.hardened1-1
linux-hardened-headers 7.0.11.hardened2-1 -> 7.0.12.hardened1-1
mesa 1:26.1.2-1 -> 1:26.1.2-2
mkinitcpio-utils 0.0.5-2 -> 0.0.6-1
mosh 1.4.0-30 -> 1.4.0-31
nss 3.124-1 -> 3.125-1
openexr 3.4.12-2 -> 3.4.12-3
openjph 0.28.1-1 -> 0.29.0-1
openmpi 5.0.10-2 -> 5.0.10-3
osv-scanner 2.3.8-1 -> 2.4.0-1
pambase 20250719-1 -> 20260616-1
pandoc-cli 3.6-42 -> 3.6-51
perl-uri 5.34-2 -> 5.35-1
procps-ng 4.0.6-1 -> 4.0.6-2
protobuf 35.0-1 -> 35.0-2
protobuf-c 1.5.2-10 -> 1.5.2-11
python-certifi 2026.05.20-1 -> 2026.06.17-1
python-docutils 1:0.22.4-1 -> 1:0.23-1
python-filelock 3.29.0-1 -> 3.29.3-1
python-joserfc 1.6.8-1 -> 1.7.1-1
python-mako 1.3.11-1 -> 1.3.12-1
python-matplotlib 3.10.9-1 -> 3.11.0-1
python-patiencediff 0.2.18-2 -> 0.2.19-1
python-protobuf 35.0-1 -> 35.0-2
python-pytorch 2.12.0-3 -> 2.12.0-5
python-sqlalchemy 2.0.50-1 -> 2.0.51-1
python-tornado 6.5.6-1 -> 6.5.7-1
python-virtualenv 21.4.3-1 -> 21.5.0-1
python-wrapt 2.1.2-1 -> 2.2.1-1
python-zope-interface 8.4-1 -> 8.5-1
qt6-webengine 6.11.1-3 -> 6.11.1-4
rabbitmqadmin 1:2.31.0-1 -> 1:2.32.0-1
re2 2:2025.11.05-4 -> 2:2025.11.05-5
ripgrep 15.1.0-3 -> 15.1.0-4
semver 7.8.1-1 -> 7.8.4-1
shadow 4.18.0-1 -> 4.19.4.arch1-1
shellcheck 0.11.0-110 -> 0.11.0-112
sudo 1.9.17.p2-2 -> 1.9.17.p2-6
tig 2.6.0-1 -> 2.6.1-1
util-linux 2.42.1-1 -> 2.42.2-1
util-linux-libs 2.42.1-1 -> 2.42.2-1
vapoursynth 76-1 -> 77-1
vifm 0.14.3-2 -> 0.14.4-1
vim-runtime 9.2.0623-1 -> 9.2.0670-1
vulkan-mesa-implicit-layers 1:26.1.2-1 -> 1:26.1.2-2
vulkan-radeon 1:26.1.2-1 -> 1:26.1.2-2
xdg-desktop-portal 1.22.0-1 -> 1.22.1-1
";
        let updates = parse(data);
        assert_eq!(
            updates,
            vec![
                Update {
                    name: "libnvme".to_string()
                },
                Update {
                    name: "lighttpd".to_string()
                },
                Update {
                    name: "linux-api-headers".to_string()
                },
                Update {
                    name: "linux-hardened".to_string()
                },
                Update {
                    name: "linux-hardened-docs".to_string()
                },
                Update {
                    name: "linux-hardened-headers".to_string()
                },
                Update {
                    name: "mesa".to_string()
                },
                Update {
                    name: "mkinitcpio-utils".to_string()
                },
                Update {
                    name: "mosh".to_string()
                },
                Update {
                    name: "nss".to_string()
                },
                Update {
                    name: "openexr".to_string()
                },
                Update {
                    name: "openjph".to_string()
                },
                Update {
                    name: "openmpi".to_string()
                },
                Update {
                    name: "osv-scanner".to_string()
                },
                Update {
                    name: "pambase".to_string()
                },
                Update {
                    name: "pandoc-cli".to_string()
                },
                Update {
                    name: "perl-uri".to_string()
                },
                Update {
                    name: "procps-ng".to_string()
                },
                Update {
                    name: "protobuf".to_string()
                },
                Update {
                    name: "protobuf-c".to_string()
                },
                Update {
                    name: "python-certifi".to_string()
                },
                Update {
                    name: "python-docutils".to_string()
                },
                Update {
                    name: "python-filelock".to_string()
                },
                Update {
                    name: "python-joserfc".to_string()
                },
                Update {
                    name: "python-mako".to_string()
                },
                Update {
                    name: "python-matplotlib".to_string()
                },
                Update {
                    name: "python-patiencediff".to_string()
                },
                Update {
                    name: "python-protobuf".to_string()
                },
                Update {
                    name: "python-pytorch".to_string()
                },
                Update {
                    name: "python-sqlalchemy".to_string()
                },
                Update {
                    name: "python-tornado".to_string()
                },
                Update {
                    name: "python-virtualenv".to_string()
                },
                Update {
                    name: "python-wrapt".to_string()
                },
                Update {
                    name: "python-zope-interface".to_string()
                },
                Update {
                    name: "qt6-webengine".to_string()
                },
                Update {
                    name: "rabbitmqadmin".to_string()
                },
                Update {
                    name: "re2".to_string()
                },
                Update {
                    name: "ripgrep".to_string()
                },
                Update {
                    name: "semver".to_string()
                },
                Update {
                    name: "shadow".to_string()
                },
                Update {
                    name: "shellcheck".to_string()
                },
                Update {
                    name: "sudo".to_string()
                },
                Update {
                    name: "tig".to_string()
                },
                Update {
                    name: "util-linux".to_string()
                },
                Update {
                    name: "util-linux-libs".to_string()
                },
                Update {
                    name: "vapoursynth".to_string()
                },
                Update {
                    name: "vifm".to_string()
                },
                Update {
                    name: "vim-runtime".to_string()
                },
                Update {
                    name: "vulkan-mesa-implicit-layers".to_string()
                },
                Update {
                    name: "vulkan-radeon".to_string()
                },
                Update {
                    name: "xdg-desktop-portal".to_string()
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
