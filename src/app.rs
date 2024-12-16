use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use matrix_sdk::{
    AuthSession, ServerName,
    encryption::{BackupDownloadStrategy, EncryptionSettings, VerificationState},
    matrix_auth::MatrixSession,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::{fs, sync::mpsc};
use tracing::{info, instrument, warn};

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
    tx: mpsc::Sender<Event>,
}

#[derive(Debug, Clone)]
enum Event {
    Verification(VerificationState),
}

impl App {
    pub async fn start(config_p: &Path) -> Result<Self> {
        info!("Starting app");
        let bs = fs::read(config_p).await.context("read config")?;
        let config: Config = serde_yaml::from_slice(bs.as_slice()).context("parse config yaml")?;
        let client = matrix_sdk::Client::builder()
            .homeserver_url(&config.homeserver_url)
            .sqlite_store(&config.db_path, None)
            .with_encryption_settings(EncryptionSettings {
                backup_download_strategy: BackupDownloadStrategy::AfterDecryptionFailure,
                ..Default::default()
            })
            .build()
            .await
            .context("build client")?;
        let (tx, rx) = mpsc::channel(1024);
        let app = App { config, client, tx };
        app.auth().await.context("auth")?;

        info!("Spawning verification listener");
        tokio::spawn(app.clone().verification_listener());

        info!("Spawning controller");
        tokio::spawn(app.clone().controller(rx));

        info!("App initialized");
        Ok(app)
    }

    // the main control task
    #[instrument(skip_all)]
    async fn controller(self, mut rx: mpsc::Receiver<Event>) {
        while let Some(ev) = rx.recv().await {
            info!("Event: {ev:?}");
        }
    }

    async fn verification_listener(self) {
        let mut vs = self.client.encryption().verification_state();
        while let Some(vs) = vs.next().await {
            if self
                .tx
                .send(Event::Verification(vs))
                .await
                .is_err()
            {
                break;
            }
        }
    }

    async fn auth(&self) -> Result<()> {
        info!("Initializing auth");
        match self.restore_session().await {
            Ok(true) => {
                info!("Session restored");
                return Ok(());
            }
            Ok(false) => info!("No session was found"),
            Err(err) => warn!("Session restore failed: {err}. Falling back to login"),
        };
        self.login().await.context("login")
    }

    async fn login(&self) -> Result<()> {
        info!("Attempting login");
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
        info!("Restoring session");
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
