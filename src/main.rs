#![allow(unused)]

use clap::Parser;
use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
struct Args {
    #[arg(long, default_value = "config.yaml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    matrix_client::app::App::start(&args.config).await?;
    Ok(())
}
