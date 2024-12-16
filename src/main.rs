#![allow(unused)]

use clap::Parser;
use matrix_client::{
    app::{App, Config},
    obs,
};
use std::{future, path::PathBuf, time::Duration};
use tracing::info;

#[derive(clap::Parser, Debug)]
struct Args {
    #[arg(long, default_value = "matrix-client.log")]
    log: PathBuf,
    #[arg(long, default_value = "config.yaml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let jh = tokio::task::spawn(run());
    jh.await?
}

async fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    let _guard = obs::init(&args.log)?;
    let app = App::start(&args.config).await?;
    info!("Application started");
    future::pending::<()>().await;
    Ok(())
}
