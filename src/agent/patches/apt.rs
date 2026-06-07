use crate::errors::*;
use tokio::fs;
use tokio::io::ErrorKind;

const PATH: &str = "/var/lib/dpkg/status";

fn parse(data: &str) -> Vec<String> {
    let data = data.strip_prefix("Listing...\n").unwrap_or(data);

    data.lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(|s| {
            let (pkg, _component) = s.split_once('/').unwrap_or((s, ""));
            // TODO: security: check if component ends with `-security`
            // TODO: new version: $2
            pkg.to_string()
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

pub async fn run() -> Result<()> {
    if !detect().await {
        warn!("apt database not found, skipping");
        return Ok(());
    }

    let update = tokio::process::Command::new("apt")
        .arg("update")
        .output()
        .await
        .context("Failed to run apt update")?;
    if !update.status.success() {
        bail!(
            "apt update failed: {:?}",
            String::from_utf8_lossy(&update.stderr)
        );
    }

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
    let updates = parse(&stdout);
    for update in updates {
        info!("Update available: {update:?}");
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
                "adduser",
                "apt",
                "base-files",
                "base-passwd",
                "bash",
                "bsdutils",
                "coreutils",
                "dash",
                "debconf",
                "debian-archive-keyring",
                "debianutils",
                "diffutils",
                "dpkg",
                "e2fsprogs",
                "findutils",
                "gcc-12-base",
                "gpgv",
                "grep",
                "gzip",
                "hostname",
                "init-system-helpers",
                "libacl1",
                "libattr1",
                "libaudit-common",
                "libaudit1",
                "libblkid1",
                "libbz2-1.0",
                "libc-bin",
                "libc6",
                "libcap-ng0",
                "libcap2",
                "libcom-err2",
                "libcrypt1",
                "libdebconfclient0",
                "libffi8",
                "libgcc-s1",
                "libgcrypt20",
                "libgmp10",
                "libgpg-error0",
                "libidn2-0",
                "liblz4-1",
                "liblzma5",
                "libmd0",
                "libmount1",
                "libp11-kit0",
                "libpam-modules-bin",
                "libpam-modules",
                "libpam-runtime",
                "libpam0g",
                "libpcre2-8-0",
                "libseccomp2",
                "libselinux1",
                "libsemanage-common",
                "libsemanage2",
                "libsepol2",
                "libsmartcols1",
                "libss2",
                "libstdc++6",
                "libsystemd0",
                "libtasn1-6",
                "libtinfo6",
                "libudev1",
                "libuuid1",
                "libxxhash0",
                "libzstd1",
                "login",
                "logsave",
                "mawk",
                "mount",
                "ncurses-base",
                "ncurses-bin",
                "passwd",
                "perl-base",
                "sed",
                "sysvinit-utils",
                "tar",
                "tzdata",
                "usr-is-merged",
                "util-linux-extra",
                "util-linux",
                "zlib1g",
            ]
        );
    }

    #[test]
    fn test_parse_empty() {
        let updates = parse("");
        assert_eq!(updates, Vec::<String>::new());
    }
}
