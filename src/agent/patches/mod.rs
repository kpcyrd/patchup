pub mod apk;
pub mod apt;

#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UpdateStatus {
    pub pending: Vec<Update>,
    pub refresh_error: bool,
}
