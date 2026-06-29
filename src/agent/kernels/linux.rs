use crate::agent::kernels;
use crate::errors::*;
use tokio::fs;

const MODULE_PATH: &str = "/lib/modules";

pub async fn list_available() -> Result<Vec<kernels::sort::Version>> {
    let mut dir = match fs::read_dir(MODULE_PATH).await {
        Ok(dir) => dir,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            debug!("No modules directory found at {MODULE_PATH:?}");
            return Ok(Default::default());
        }
        Err(err) => {
            return Err(Error::from(err).context(format!(
                "Failed to read modules directory at {MODULE_PATH:?}"
            )));
        }
    };

    let mut available = Vec::new();
    while let Some(entry) = dir.next_entry().await? {
        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };

        if name == "." || name == ".." {
            continue;
        }

        if let Ok(version) = name.parse::<kernels::sort::Version>() {
            debug!("Found kernel version on disk: {version:?}");
            available.push(version);
        }
    }

    Ok(available)
}
