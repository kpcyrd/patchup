use crate::agent::patches::{Update, UpdateStatus};
use crate::args::Output;
use crate::errors::*;
use tokio::fs;
use tokio::io::ErrorKind;

pub const ID: &str = "apt";
const PATH: &str = "/var/lib/dpkg/status";

fn parse(data: &str) -> Vec<Update> {
    let data = data.strip_prefix("Listing...\n").unwrap_or(data);

    data.lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(|s| {
            let (pkg, _component) = s.split_once('/').unwrap_or((s, ""));
            // TODO: security: check if component ends with `-security`
            // TODO: new version: $2
            Update {
                name: pkg.to_string(),
            }
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

    debug!("Running apt update");
    let update = tokio::process::Command::new("apt")
        .arg("update")
        .output()
        .await
        .context("Failed to run apt update")?;
    if !update.status.success() {
        warn!(
            "apt update failed: {:?}",
            String::from_utf8_lossy(&update.stderr)
        );
        status.refresh_error = true;
    }

    debug!("Running apt list --upgradable");
    let list = tokio::process::Command::new("apt")
        .args(["list", "--upgradable"])
        .output()
        .await
        .context("Failed to run apt list")?;
    if !list.status.success() {
        bail!(
            "apt list failed: {:?}",
            String::from_utf8_lossy(&list.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&list.stdout);
    status.pending = parse(&stdout);

    Ok(status)
}

pub async fn run(output: &Output) -> Result<()> {
    if !detect().await {
        warn!("apt database not found, skipping");
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
        let data = "Listing...
adduser/stable 3.152 all [upgradable from: 3.134]
apt/stable 3.0.3 amd64 [upgradable from: 2.6.1]
base-files/stable 13.8+deb13u5 amd64 [upgradable from: 12.4+deb12u7]
base-passwd/stable 3.6.7 amd64 [upgradable from: 3.6.1]
bash/stable 5.2.37-2+b9 amd64 [upgradable from: 5.2.15-2+b7]
bsdutils/stable 1:2.41-5 amd64 [upgradable from: 1:2.38.1-5+deb12u1]
coreutils/stable 9.7-3 amd64 [upgradable from: 9.1-1]
dash/stable 0.5.12-12 amd64 [upgradable from: 0.5.12-2]
debconf/stable 1.5.91 all [upgradable from: 1.5.82]
debian-archive-keyring/stable 2025.1 all [upgradable from: 2023.3+deb12u1]
debianutils/stable 5.23.2 amd64 [upgradable from: 5.7-0.5~deb12u1]
diffutils/stable 1:3.10-4 amd64 [upgradable from: 1:3.8-4]
dpkg/stable 1.22.22 amd64 [upgradable from: 1.21.22]
e2fsprogs/stable 1.47.2-3+b11 amd64 [upgradable from: 1.47.0-2]
findutils/stable 4.10.0-3 amd64 [upgradable from: 4.9.0-4]
gcc-12-base/stable 12.4.0-5 amd64 [upgradable from: 12.2.0-14]
gpgv/stable 2.4.7-21+deb13u1+b3 amd64 [upgradable from: 2.2.40-1.1]
grep/stable 3.11-4 amd64 [upgradable from: 3.8-5]
gzip/stable 1.13-1 amd64 [upgradable from: 1.12-1]
hostname/stable 3.25 amd64 [upgradable from: 3.23+nmu1]
init-system-helpers/stable 1.69~deb13u1 all [upgradable from: 1.65.2]
libacl1/stable 2.3.2-2+b1 amd64 [upgradable from: 2.3.1-3]
libattr1/stable 1:2.5.2-3 amd64 [upgradable from: 1:2.5.1-4]
libaudit-common/stable 1:4.0.2-2 all [upgradable from: 1:3.0.9-1]
libaudit1/stable 1:4.0.2-2+b2 amd64 [upgradable from: 1:3.0.9-1]
libblkid1/stable 2.41-5 amd64 [upgradable from: 2.38.1-5+deb12u1]
libbz2-1.0/stable 1.0.8-6 amd64 [upgradable from: 1.0.8-5+b1]
libc-bin/stable 2.41-12+deb13u3 amd64 [upgradable from: 2.36-9+deb12u8]
libc6/stable 2.41-12+deb13u3 amd64 [upgradable from: 2.36-9+deb12u8]
libcap-ng0/stable 0.8.5-4+b1 amd64 [upgradable from: 0.8.3-1+b3]
libcap2/stable 1:2.75-10+deb13u1+b1 amd64 [upgradable from: 1:2.66-4]
libcom-err2/stable 1.47.2-3+b11 amd64 [upgradable from: 1.47.0-2]
libcrypt1/stable 1:4.4.38-1 amd64 [upgradable from: 1:4.4.33-2]
libdebconfclient0/stable 0.280 amd64 [upgradable from: 0.270]
libffi8/stable 3.4.8-2 amd64 [upgradable from: 3.4.4-1]
libgcc-s1/stable 14.2.0-19 amd64 [upgradable from: 12.2.0-14]
libgcrypt20/stable-security 1.11.0-7+deb13u1 amd64 [upgradable from: 1.10.1-3]
libgmp10/stable 2:6.3.0+dfsg-3 amd64 [upgradable from: 2:6.2.1+dfsg1-1.1]
libgpg-error0/stable 1.51-4 amd64 [upgradable from: 1.46-1]
libidn2-0/stable 2.3.8-2 amd64 [upgradable from: 2.3.3-1+b1]
liblz4-1/stable 1.10.0-4 amd64 [upgradable from: 1.9.4-1]
liblzma5/stable 5.8.1-1 amd64 [upgradable from: 5.4.1-0.2]
libmd0/stable 1.1.0-2+b1 amd64 [upgradable from: 1.0.4-2]
libmount1/stable 2.41-5 amd64 [upgradable from: 2.38.1-5+deb12u1]
libp11-kit0/stable 0.25.5-3 amd64 [upgradable from: 0.24.1-2]
libpam-modules-bin/stable 1.7.0-5 amd64 [upgradable from: 1.5.2-6+deb12u1]
libpam-modules/stable 1.7.0-5 amd64 [upgradable from: 1.5.2-6+deb12u1]
libpam-runtime/stable 1.7.0-5 all [upgradable from: 1.5.2-6+deb12u1]
libpam0g/stable 1.7.0-5 amd64 [upgradable from: 1.5.2-6+deb12u1]
libpcre2-8-0/stable 10.46-1~deb13u1 amd64 [upgradable from: 10.42-1]
libseccomp2/stable 2.6.0-2 amd64 [upgradable from: 2.5.4-1+deb12u1]
libselinux1/stable 3.8.1-1 amd64 [upgradable from: 3.4-1+b6]
libsemanage-common/stable 3.8.1-1 all [upgradable from: 3.4-1]
libsemanage2/stable 3.8.1-1 amd64 [upgradable from: 3.4-1+b5]
libsepol2/stable 3.8.1-1 amd64 [upgradable from: 3.4-2.1]
libsmartcols1/stable 2.41-5 amd64 [upgradable from: 2.38.1-5+deb12u1]
libss2/stable 1.47.2-3+b11 amd64 [upgradable from: 1.47.0-2]
libstdc++6/stable 14.2.0-19 amd64 [upgradable from: 12.2.0-14]
libsystemd0/stable 257.13-1~deb13u1 amd64 [upgradable from: 252.30-1~deb12u2]
libtasn1-6/stable 4.20.0-2 amd64 [upgradable from: 4.19.0-2]
libtinfo6/stable 6.5+20250216-2 amd64 [upgradable from: 6.4-4]
libudev1/stable 257.13-1~deb13u1 amd64 [upgradable from: 252.30-1~deb12u2]
libuuid1/stable 2.41-5 amd64 [upgradable from: 2.38.1-5+deb12u1]
libxxhash0/stable 0.8.3-2 amd64 [upgradable from: 0.8.1-1]
libzstd1/stable 1.5.7+dfsg-1 amd64 [upgradable from: 1.5.4+dfsg2-5]
login/stable 1:4.16.0-2+really2.41-5 amd64 [upgradable from: 1:4.13+dfsg1-1+b1]
logsave/stable 1.47.2-3+b11 amd64 [upgradable from: 1.47.0-2]
mawk/stable 1.3.4.20250131-1 amd64 [upgradable from: 1.3.4.20200120-3.1]
mount/stable 2.41-5 amd64 [upgradable from: 2.38.1-5+deb12u1]
ncurses-base/stable 6.5+20250216-2 all [upgradable from: 6.4-4]
ncurses-bin/stable 6.5+20250216-2 amd64 [upgradable from: 6.4-4]
passwd/stable 1:4.17.4-2 amd64 [upgradable from: 1:4.13+dfsg1-1+b1]
perl-base/stable 5.40.1-6 amd64 [upgradable from: 5.36.0-7+deb12u1]
sed/stable 4.9-2+deb13u1 amd64 [upgradable from: 4.9-1]
sysvinit-utils/stable 3.14-4 amd64 [upgradable from: 3.06-4]
tar/stable 1.35+dfsg-3.1 amd64 [upgradable from: 1.34+dfsg-1.2+deb12u1]
tzdata/stable 2026b-0+deb13u1 all [upgradable from: 2024a-0+deb12u1]
usr-is-merged/stable 39+nmu2 all [upgradable from: 37~deb12u1]
util-linux-extra/stable 2.41-5 amd64 [upgradable from: 2.38.1-5+deb12u1]
util-linux/stable 2.41-5 amd64 [upgradable from: 2.38.1-5+deb12u1]
zlib1g/stable 1:1.3.dfsg+really1.3.1-1+b1 amd64 [upgradable from: 1:1.2.13.dfsg-1]
";
        let updates = parse(data);
        assert_eq!(
            updates,
            vec![
                Update {
                    name: "adduser".to_string()
                },
                Update {
                    name: "apt".to_string()
                },
                Update {
                    name: "base-files".to_string()
                },
                Update {
                    name: "base-passwd".to_string()
                },
                Update {
                    name: "bash".to_string()
                },
                Update {
                    name: "bsdutils".to_string()
                },
                Update {
                    name: "coreutils".to_string()
                },
                Update {
                    name: "dash".to_string()
                },
                Update {
                    name: "debconf".to_string()
                },
                Update {
                    name: "debian-archive-keyring".to_string()
                },
                Update {
                    name: "debianutils".to_string()
                },
                Update {
                    name: "diffutils".to_string()
                },
                Update {
                    name: "dpkg".to_string()
                },
                Update {
                    name: "e2fsprogs".to_string()
                },
                Update {
                    name: "findutils".to_string()
                },
                Update {
                    name: "gcc-12-base".to_string()
                },
                Update {
                    name: "gpgv".to_string()
                },
                Update {
                    name: "grep".to_string()
                },
                Update {
                    name: "gzip".to_string()
                },
                Update {
                    name: "hostname".to_string()
                },
                Update {
                    name: "init-system-helpers".to_string()
                },
                Update {
                    name: "libacl1".to_string()
                },
                Update {
                    name: "libattr1".to_string()
                },
                Update {
                    name: "libaudit-common".to_string()
                },
                Update {
                    name: "libaudit1".to_string()
                },
                Update {
                    name: "libblkid1".to_string()
                },
                Update {
                    name: "libbz2-1.0".to_string()
                },
                Update {
                    name: "libc-bin".to_string()
                },
                Update {
                    name: "libc6".to_string()
                },
                Update {
                    name: "libcap-ng0".to_string()
                },
                Update {
                    name: "libcap2".to_string()
                },
                Update {
                    name: "libcom-err2".to_string()
                },
                Update {
                    name: "libcrypt1".to_string()
                },
                Update {
                    name: "libdebconfclient0".to_string()
                },
                Update {
                    name: "libffi8".to_string()
                },
                Update {
                    name: "libgcc-s1".to_string()
                },
                Update {
                    name: "libgcrypt20".to_string()
                },
                Update {
                    name: "libgmp10".to_string()
                },
                Update {
                    name: "libgpg-error0".to_string()
                },
                Update {
                    name: "libidn2-0".to_string()
                },
                Update {
                    name: "liblz4-1".to_string()
                },
                Update {
                    name: "liblzma5".to_string()
                },
                Update {
                    name: "libmd0".to_string()
                },
                Update {
                    name: "libmount1".to_string()
                },
                Update {
                    name: "libp11-kit0".to_string()
                },
                Update {
                    name: "libpam-modules-bin".to_string()
                },
                Update {
                    name: "libpam-modules".to_string()
                },
                Update {
                    name: "libpam-runtime".to_string()
                },
                Update {
                    name: "libpam0g".to_string()
                },
                Update {
                    name: "libpcre2-8-0".to_string()
                },
                Update {
                    name: "libseccomp2".to_string()
                },
                Update {
                    name: "libselinux1".to_string()
                },
                Update {
                    name: "libsemanage-common".to_string()
                },
                Update {
                    name: "libsemanage2".to_string()
                },
                Update {
                    name: "libsepol2".to_string()
                },
                Update {
                    name: "libsmartcols1".to_string()
                },
                Update {
                    name: "libss2".to_string()
                },
                Update {
                    name: "libstdc++6".to_string()
                },
                Update {
                    name: "libsystemd0".to_string()
                },
                Update {
                    name: "libtasn1-6".to_string()
                },
                Update {
                    name: "libtinfo6".to_string()
                },
                Update {
                    name: "libudev1".to_string()
                },
                Update {
                    name: "libuuid1".to_string()
                },
                Update {
                    name: "libxxhash0".to_string()
                },
                Update {
                    name: "libzstd1".to_string()
                },
                Update {
                    name: "login".to_string()
                },
                Update {
                    name: "logsave".to_string()
                },
                Update {
                    name: "mawk".to_string()
                },
                Update {
                    name: "mount".to_string()
                },
                Update {
                    name: "ncurses-base".to_string()
                },
                Update {
                    name: "ncurses-bin".to_string()
                },
                Update {
                    name: "passwd".to_string()
                },
                Update {
                    name: "perl-base".to_string()
                },
                Update {
                    name: "sed".to_string()
                },
                Update {
                    name: "sysvinit-utils".to_string()
                },
                Update {
                    name: "tar".to_string()
                },
                Update {
                    name: "tzdata".to_string()
                },
                Update {
                    name: "usr-is-merged".to_string()
                },
                Update {
                    name: "util-linux-extra".to_string()
                },
                Update {
                    name: "util-linux".to_string()
                },
                Update {
                    name: "zlib1g".to_string()
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
