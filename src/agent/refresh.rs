use crate::errors::*;
use crate::ipc;
use std::path::Path;

pub async fn offer(path: &Path, mandatory: bool) -> Result<()> {
    /*
    let req = crate::ipc::agent::OfferRefreshRequest { mandatory };
    self.send_request("offer_refresh", req).await?;
    */

    info!("Sending refresh offer to agent (mandatory={})", mandatory);
    let mut sock = ipc::agent::AgentIpc::connect(path).await?;
    sock.offer_refresh(mandatory).await?;

    Ok(())
}
