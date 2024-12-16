use anyhow::{Context, Result, bail};
use matrix_sdk::{AuthSession, ServerName, matrix_auth::MatrixSession};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{info, warn};

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
        app.auth().await.context("auth")?;
        Ok(app)
    }

    async fn auth(&self) -> Result<()> {
        let f = self.restore_session().await;
        match self.restore_session().await {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(err) => warn!("Session restore failed: {err}. Falling back to login"),
        };
        self.login().await.context("login")
    }

    async fn login(&self) -> Result<()> {
        let resp = self
            .client
            .matrix_auth()
            .login_username(&self.config.username, &self.config.password)
            .initial_device_display_name("collin-matrix-client")
            .await
            .context("matrix auth")?;
        info!("Login resp: {resp:#?}");
        let session = self
            .client
            .session()
            .context("no session after login")?;
        match session {
            AuthSession::Matrix(session) => {
                let s = serde_yaml::to_string(&session).context("serialize session")?;
                fs::write(&self.config.session_path, s.as_bytes())
                    .await
                    .context("write session")?;
            }
            _ => bail!("unknown session typ: {session:?}"),
        }
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
