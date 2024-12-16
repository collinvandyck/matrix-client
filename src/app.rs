use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    username: String,
    password: String,
    db_path: String,
    session_path: String,
}

pub struct App {
    config: Config,
}

impl App {
    pub async fn start(config_p: &Path) -> Result<Self> {
        let bs = tokio::fs::read(config_p)
            .await
            .context("read config")?;
        let config: Config = serde_yaml::from_slice(bs.as_slice()).context("parse yaml")?;
        Ok(Self { config })
    }
}
