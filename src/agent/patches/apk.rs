use crate::errors::*;

fn parse(data: &str) -> Vec<String> {
    data.lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(|s| s.to_string())
        .collect()
}

pub async fn run() -> Result<()> {
    let update = tokio::process::Command::new("apk")
        .arg("update")
        .output()
        .await
        .context("Failed to run apk update")?;
    if !update.status.success() {
        bail!(
            "apk update failed: {:?}",
            String::from_utf8_lossy(&update.stderr)
        );
    }

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
                "alpine-baselayout-3.7.2-r0".to_string(),
                "alpine-baselayout-data-3.7.2-r0".to_string(),
                "alpine-release-3.23.4-r0".to_string(),
                "apk-tools-3.0.6-r0".to_string(),
                "ca-certificates-20260413-r0".to_string(),
                "ca-certificates-bundle-20260413-r0".to_string(),
                "libapk-3.0.6-r0".to_string(),
                "libcrypto3-3.5.6-r0".to_string(),
                "libssl3-3.5.6-r0".to_string(),
                "musl-1.2.5-r23".to_string(),
                "musl-dev-1.2.5-r23".to_string(),
                "musl-utils-1.2.5-r23".to_string(),
                "zlib-1.3.2-r0".to_string(),
            ]
        );
    }

    #[test]
    fn test_parse_empty() {
        let updates = parse("");
        assert_eq!(updates, Vec::<String>::new());
    }
}
