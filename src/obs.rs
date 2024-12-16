use tracing_subscriber::util::SubscriberInitExt;

pub fn init() {
    tracing_subscriber::fmt().init();
}
