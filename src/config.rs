use crate::errors::*;
use etcetera::BaseStrategy;
use serde::Deserialize;
use std::borrow::Cow;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::Path;
use tokio::fs;

#[derive(Debug, Default, PartialEq, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub hub: Hub,
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
                let strategy = etcetera::choose_base_strategy()?;
                let path = strategy.config_dir().join("patchup.toml");
                Ok(Cow::Owned(path))
            })?;

        debug!("Reading config file: {path:?}");
        let config = match fs::read_to_string(&path).await {
            Ok(config) => config,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                debug!("Config file not found, using default config");
                return Ok(Default::default());
            }
            Err(err) => {
                return Err(err).with_context(|| format!("Failed to read config file: {path:?}"));
            }
        };

        let config = Self::parse(&config)
            .with_context(|| format!("Failed to parse config file: {path:?}"))?;

        Ok(config)
    }
}

#[derive(Debug, Default, PartialEq, Deserialize)]
pub struct Hub {
    pub addr: Option<SocketAddr>,
    pub pubkey: Option<String>,
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
