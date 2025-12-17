use axum::serve;
use cryptochat_node::{init_tracing, router, AppConfig, AppState};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = AppConfig::from_env()?;
    let state = AppState::new(config.clone());

    let app = router(Arc::clone(&state));

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    let listener = TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;
    info!(%local_addr, build_id = %config.build_id, "starting CryptoChat node service");

    serve(listener, app).await?;
    Ok(())
}
