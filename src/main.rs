#![allow(unused)]

use clap::Parser;
use matrix_client::{app::App, obs};
use std::path::PathBuf;
use tracing::info;

#[derive(clap::Parser, Debug)]
struct Args {
    #[arg(long, default_value = "config.yaml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    info!("Main hi");
    obs::init();
    let args = Args::parse();
    App::start(&args.config).await?;
    Ok(())
}
