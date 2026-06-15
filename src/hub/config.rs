use crate::errors::*;
use russh::keys::PublicKey;
use serde::Deserialize;
use std::borrow::Cow;
use std::net::SocketAddr;
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub system: System,
    #[serde(default)]
    pub admins: Vec<Admin>,
    #[serde(default)]
    pub nodes: Vec<Node>,
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

        info!("Loading config from: {path:?}");
        let config = fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read config file: {path:?}"))?;

        let config = Self::parse(&config)
            .with_context(|| format!("Failed to parse config file: {path:?}"))?;

        Ok(config)
    }

    pub fn is_admin(&self, public_key: &PublicKey) -> bool {
        self.admins
            .iter()
            .any(|admin| admin.keys.contains(public_key))
    }

    pub fn is_agent(&self, public_key: &PublicKey) -> bool {
        self.nodes.iter().any(|node| node.keys.contains(public_key))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct System {
    pub bind: Option<SocketAddr>,
    pub metrics: Option<SocketAddr>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Admin {
    pub name: String,
    pub keys: Vec<PublicKey>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Node {
    pub name: Option<String>,
    pub keys: Vec<PublicKey>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let config = Config::parse("").unwrap();
        assert_eq!(config, Default::default());
    }

    #[test]
    fn test_parse_admins() {
        let config = Config::parse(
            r#"
        [[admins]]
        name = "foo"
        keys = [
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBTiXsBbUK8JxM9IaZvChvYgW4e2tAPKst1VRaS5AAAA",
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH9USmOeLVHiO81A2Vt08gRgQyszmHRRG6j1hBOOAAAA",
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINn2lniQmcA9/fb7CkGptKVlgpv5UJyGeNcMvZRkAAAA",
        ]

        [[admins]]
        name = "bar"
        keys = []
        "#,
        )
        .unwrap();
        assert_eq!(
            config,
            Config {
                admins: vec![
                    Admin {
                        name: "foo".to_string(),
                        keys: vec![
                            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBTiXsBbUK8JxM9IaZvChvYgW4e2tAPKst1VRaS5AAAA".parse().unwrap(),
                            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH9USmOeLVHiO81A2Vt08gRgQyszmHRRG6j1hBOOAAAA".parse().unwrap(),
                            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINn2lniQmcA9/fb7CkGptKVlgpv5UJyGeNcMvZRkAAAA".parse().unwrap(),
                        ],
                    },
                    Admin {
                        name: "bar".to_string(),
                        keys: vec![],
                    },
                ],
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_parse_nodes() {
        let config = Config::parse(
            r#"
        [[nodes]]
        name = "foo"
        keys = [
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBTiXsBbUK8JxM9IaZvChvYgW4e2tAPKst1VRaS5AAAA",
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH9USmOeLVHiO81A2Vt08gRgQyszmHRRG6j1hBOOAAAA",
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINn2lniQmcA9/fb7CkGptKVlgpv5UJyGeNcMvZRkAAAA",
        ]

        [[nodes]]
        keys = [
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIF1riHsTvof12dYeqHD0kSjMDlk0B6yHGDgrAjx3AAAA",
        ]
        "#,
        )
        .unwrap();
        assert_eq!(
            config,
            Config {
                nodes: vec![
                    Node {
                        name: Some("foo".to_string()),
                        keys: vec![
                            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBTiXsBbUK8JxM9IaZvChvYgW4e2tAPKst1VRaS5AAAA".parse().unwrap(),
                            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIH9USmOeLVHiO81A2Vt08gRgQyszmHRRG6j1hBOOAAAA".parse().unwrap(),
                            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINn2lniQmcA9/fb7CkGptKVlgpv5UJyGeNcMvZRkAAAA".parse().unwrap(),
                        ],
                    },
                    Node {
                        name: None,
                        keys: vec![
                            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIF1riHsTvof12dYeqHD0kSjMDlk0B6yHGDgrAjx3AAAA".parse().unwrap(),
                        ],
                    },
                ],
                ..Default::default()
            }
        );
    }
}
