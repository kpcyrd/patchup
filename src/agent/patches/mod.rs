pub mod apk;
pub mod apt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Update {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateStatus {
    pub pending: Vec<Update>,
    pub refresh_error: bool,
}
