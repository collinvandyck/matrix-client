use anyhow::{Context, Ok, Result};
use matrix_sdk::{ServerName, matrix_auth::MatrixSession};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    username: String,
    password: String,
    db_path: PathBuf,
    session_path: PathBuf,
    homeserver_url: String,
}

#[derive(Clone)]
pub struct App {
    config: Config,
    client: matrix_sdk::Client,
}

impl App {
    pub async fn start(config_p: &Path) -> Result<Self> {
        info!("Starting app");
        let bs = fs::read(config_p).await.context("read config")?;
        let config: Config = serde_yaml::from_slice(bs.as_slice()).context("parse config yaml")?;
        let client = matrix_sdk::Client::builder()
            .homeserver_url(&config.homeserver_url)
            .sqlite_store(&config.db_path, None)
            .build()
            .await
            .context("build client")?;
        let app = App { config, client };
        let restored = app
            .restore_session()
            .await
            .context("restore session")?;
        if !restored {
            app.login().await.context("login")?;
        }
        Ok(app)
    }

    async fn login(&self) -> Result<()> {
        todo!()
    }

    async fn restore_session(&self) -> Result<bool> {
        if !self.config.session_path.exists() {
            return Ok(false);
        }
        let bs = fs::read(&self.config.session_path)
            .await
            .context("read session")?;
        let session: MatrixSession = serde_yaml::from_slice(bs.as_slice()).context("parse session yaml")?;
        self.client
            .restore_session(session)
            .await
            .context("restore session")?;
        Ok(true)
    }
}
