use crate::agent::State;
use crate::errors::*;
use crate::ipc::agent::Hub;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use tokio::fs;

const FILE_NAME: &str = "agent.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Storage<'a> {
    pub hub: Cow<'a, Option<Hub>>,
}

fn path(folder: &Path) -> PathBuf {
    folder.join(FILE_NAME)
}

pub(super) async fn load(state: &mut State) -> Result<()> {
    let path = path(&state.data_dir);

    debug!("Loading disk state from: {path:?}");
    let data = match fs::read(&path).await {
        Ok(buf) => buf,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            debug!("No existing storage found");
            return Ok(());
        }
        Err(err) => {
            error!("Failed to read storage file {path:?}: {err:#}");
            return Err(err.into());
        }
    };

    match serde_json::from_slice::<Storage>(&data) {
        Ok(storage) => {
            // Applying disk state to the agent's state
            state.data.hub = storage.hub.into_owned();
        }
        Err(err) => {
            warn!("Failed to parse storage file, not applying: {err:#}");
        }
    }

    Ok(())
}

pub(super) async fn save(state: &State) -> Result<()> {
    let path = path(&state.data_dir);

    let mut buf = serde_json::to_string(&Storage {
        hub: Cow::Borrowed(&state.data.hub),
    })?;
    buf.push('\n');

    debug!("Saving disk state to: {path:?}");
    if let Err(err) = fs::write(&path, buf).await {
        // Failure to write to disk shouldn't make the agent exit
        error!("Failed to write to state file {path:?}: {err:#}");
    }

    Ok(())
}
