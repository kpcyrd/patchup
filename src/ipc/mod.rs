pub mod agent;

use crate::errors::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Status,
    Refresh,
}

pub async fn send<W: AsyncWriteExt + Unpin, T: Serialize + fmt::Debug>(
    w: &mut W,
    msg: &T,
) -> Result<()> {
    debug!("Sending message: {:?}", msg);
    let mut buf = serde_json::to_string(msg)?;
    buf.push('\n');
    w.write_all(buf.as_bytes()).await?;
    w.flush().await?;
    Ok(())
}

pub async fn recv<R: AsyncBufReadExt + Unpin, T: DeserializeOwned + fmt::Debug>(
    r: &mut R,
) -> Result<T> {
    recv_opt(r)
        .await?
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF").into())
}

pub async fn recv_opt<R: AsyncBufReadExt + Unpin, T: DeserializeOwned + fmt::Debug>(
    r: &mut R,
) -> Result<Option<T>> {
    let mut line = String::new();

    match r.read_line(&mut line).await {
        Ok(_) if line.is_empty() => {
            debug!("Received EOF while reading from socket");
            Ok(None)
        }
        Ok(_) => {
            let msg = serde_json::from_str::<T>(&line)?;
            debug!("Received message: {:?}", msg);
            Ok(Some(msg))
        }
        // TODO: check if this branch is necessary
        /*
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => {
            debug!("Received EOF while reading from socket");
            Ok(None)
        }
        */
        Err(err) => Err(err.into()),
    }
}
