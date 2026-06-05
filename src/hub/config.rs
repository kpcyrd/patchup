use crate::errors::*;
use serde::Deserialize;
use std::borrow::Cow;
use std::net::SocketAddr;
use std::path::Path;
use tokio::fs;

#[derive(Debug, Default, PartialEq, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub system: System,
    #[serde(default)]
    pub admins: Vec<Admin>,
    #[serde(default)]
    pub hosts: Vec<Host>,
}

impl Config {
    pub fn parse(config: &str) -> Result<Self> {
        let config = toml::from_str(config)?;
        Ok(config)
    }

    pub async fn load(path: Option<&Path>) -> Result<Self> {
        let path = path
            .map(|p| anyhow::Ok(Cow::Borrowed(p)))
            .unwrap_or_else(|| {
                let path = Path::new("/etc/patchup/hub.toml");
                Ok(Cow::Borrowed(path))
            })?;

        let config = fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read config file: {path:?}"))?;

        let config = Self::parse(&config)
            .with_context(|| format!("Failed to parse config file: {path:?}"))?;

        Ok(config)
    }
}

#[derive(Debug, Default, PartialEq, Deserialize)]
pub struct System {
    pub bind: Option<SocketAddr>,
}

#[derive(Debug, Default, PartialEq, Deserialize)]
pub struct Admin {
    pub name: String,
    pub keys: Vec<String>, // TODO: change type
}

#[derive(Debug, Default, PartialEq, Deserialize)]
pub struct Host {
    pub name: String,
    pub keys: Vec<String>, // TODO: change type
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let config = Config::parse("").unwrap();
        assert_eq!(config, Default::default());
    }
}
