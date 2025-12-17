pub mod config;
pub mod overlay;
pub mod messaging;
pub mod routes;
pub mod state;
pub mod storage;

pub use config::AppConfig;
pub use routes::router;
pub use state::AppState;

#[cfg(target_os = "android")]
pub mod android;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

pub fn init_tracing() {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}
