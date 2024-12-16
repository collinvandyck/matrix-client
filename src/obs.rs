use anyhow::{Context, Result};
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct Guard {
    _guard: WorkerGuard,
}

pub fn init(log: &Path) -> Result<Guard> {
    let ef = tracing_subscriber::EnvFilter::new("error,matrix_client=info");
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_thread_ids(false)
        .with_target(true)
        .with_file(true)
        .with_ansi(true)
        .with_line_number(true)
        .without_time();
    let (file_layer, _guard) = {
        let dir = log.parent().context("no parent dir for log")?;
        let name = log.file_name().context("no file name for log")?;
        let appender = tracing_appender::rolling::never(&dir, &name);
        let (writer, _guard) = tracing_appender::non_blocking(appender);
        let writer = tracing_subscriber::fmt::layer()
            .with_writer(writer)
            .with_thread_ids(false)
            .with_target(true)
            .with_file(true)
            .with_ansi(false)
            .with_line_number(true);
        (writer, _guard)
    };
    tracing_subscriber::registry()
        .with(ef)
        .with(stderr_layer)
        .with(file_layer)
        .init();
    Ok(Guard { _guard })
}
