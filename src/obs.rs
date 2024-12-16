use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn init() {
    let ef = tracing_subscriber::EnvFilter::new("error,matrix_client=info");
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_thread_ids(false)
        .with_target(true)
        .with_file(true)
        .with_ansi(true)
        .with_line_number(true)
        .without_time();
    tracing_subscriber::registry()
        .with(ef)
        .with(stderr_layer)
        .init();
}
