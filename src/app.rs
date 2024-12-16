use anyhow::{Context, Result, bail};
use futures_util::{StreamExt, pin_mut};
use matrix_sdk::{
    AuthSession, Client, ServerName,
    config::SyncSettings,
    encryption::{BackupDownloadStrategy, EncryptionSettings, VerificationState},
    matrix_auth::MatrixSession,
    ruma::events::{
        key::verification::request::ToDeviceKeyVerificationRequestEvent, room::message::OriginalSyncRoomMessageEvent,
    },
};
use matrix_sdk_ui::{
    RoomListService,
    eyeball_im::VectorDiff,
    room_list_service::{self, RoomList, filters::new_filter_non_left},
    sync_service::{self, SyncService},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
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
    client: Client,
    tx: mpsc::Sender<Event>,
}

#[derive(Debug)]
enum Event {
    VerificationState(VerificationState),
    SyncRoom(OriginalSyncRoomMessageEvent),
    VerificationRequest(ToDeviceKeyVerificationRequestEvent),
    SyncServiceState(sync_service::State),
    RoomDiff(Vec<VectorDiff<room_list_service::Room>>),
    FatalMatrixErr(matrix_sdk::Error),
}

impl App {
    pub async fn start(config_p: &Path) -> Result<Self> {
        info!("Starting app");
        let bs = fs::read(config_p).await.context("read config")?;
        let config: Config = serde_yaml::from_slice(bs.as_slice()).context("parse config yaml")?;
        let (tx, rx) = mpsc::channel(1024);
        let client = Client::builder()
            .homeserver_url(&config.homeserver_url)
            .sqlite_store(&config.db_path, None)
            .with_encryption_settings(EncryptionSettings {
                backup_download_strategy: BackupDownloadStrategy::AfterDecryptionFailure,
                ..Default::default()
            })
            .build()
            .await
            .context("build client")?;

        let app = App { config, client, tx };
        app.auth().await.context("auth")?;
        app.register_event_handlers();
        tokio::spawn(app.clone().verification_listener());
        tokio::spawn(app.clone().controller(rx));
        app.setup_sync().await.context("setup sync")?;
        info!("App initialized");
        Ok(app)
    }

    // the main control task
    #[instrument(skip_all)]
    async fn controller(self, mut rx: mpsc::Receiver<Event>) {
        info!("Controller task starting");
        while let Some(ev) = rx.recv().await {
            info!("Event: {ev:?}");
        }
    }

    fn register_event_handlers(&self) {
        info!("Registering event handlers");
        macro_rules! event {
            ($ev:ident, $wrap:expr) => {{
                let tx = self.tx.clone();
                self.client
                    .add_event_handler(move |ev: $ev, _: Client| {
                        async move {
                            let ev = $wrap(ev);
                            if let Err(err) = tx.send(ev).await {
                                warn!("could not send event to control thread: {err}");
                            }
                        }
                    });
            }};
        }
        event!(OriginalSyncRoomMessageEvent, Event::SyncRoom);
        event!(ToDeviceKeyVerificationRequestEvent, Event::VerificationRequest);
    }

    async fn setup_sync(&self) -> Result<()> {
        info!("Setting up sync");
        let settings = SyncSettings::default();
        let sync_service = SyncService::builder(self.client.clone())
            .build()
            .await
            .context("build sync service")?;
        let state = sync_service.state();
        tokio::spawn(self.clone().sync_state_listener(state));
        let room_list_service = sync_service.room_list_service();
        let all_rooms = room_list_service
            .all_rooms()
            .await
            .context("get all rooms listener")?;
        tokio::spawn(self.clone().room_list_listener(all_rooms));
        info!("Starting sync service");
        sync_service.start().await;
        info!("Performing first client sync");
        self.client
            .sync_once(settings.clone())
            .await
            .context("first client sync")?;
        let sync = self.clone();
        tokio::spawn(async move {
            if let Err(err) = sync.client.sync(settings).await {
                let _ = sync.tx.send(Event::FatalMatrixErr(err)).await;
            }
        });
        Ok(())
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

    async fn room_list_listener(self, rooms: RoomList) {
        info!("Starting room list listener");
        let (stream, controller) = rooms.entries_with_dynamic_adapters(5);
        controller.set_filter(Box::new(new_filter_non_left()));
        pin_mut!(stream);
        while let Some(diffs) = stream.next().await {
            let ev = Event::RoomDiff(diffs);
            if self.tx.send(ev).await.is_err() {
                break;
            }
        }
    }

    async fn sync_state_listener(self, mut state: eyeball::Subscriber<sync_service::State>) {
        info!("Starting sync state listener");
        while let Some(state) = state.next().await {
            if self
                .tx
                .send(Event::SyncServiceState(state))
                .await
                .is_err()
            {
                break;
            }
        }
    }

    async fn verification_listener(self) {
        info!("Starting verification listener");
        let mut vs = self.client.encryption().verification_state();
        while let Some(vs) = vs.next().await {
            if self
                .tx
                .send(Event::VerificationState(vs))
                .await
                .is_err()
            {
                break;
            }
        }
    }
}
