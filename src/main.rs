#![allow(unused)]

use clap::Parser;
use matrix_client::{app::App, obs};
use std::{path::PathBuf, time::Duration};
use tracing::info;

#[derive(clap::Parser, Debug)]
struct Args {
    #[arg(long, default_value = "config.yaml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let jh = tokio::task::spawn(run());
    jh.await?
}

async fn run() -> anyhow::Result<()> {
    obs::init();
    let args = Args::parse();
    let app = App::start(&args.config).await?;
    info!("Application started");
    std::future::pending::<()>().await;
    Ok(())
}
