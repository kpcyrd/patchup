use russh::SshId;
use std::borrow::Cow;
use std::time::Duration;

pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(90);
pub const KEEPALIVE_MAX: u32 = 2;

pub const ID: SshId = SshId::Standard(Cow::Borrowed("SSH-2.0-flowers-are-blooming-in-antarctica"));
