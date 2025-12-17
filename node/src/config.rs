use std::env;
use uuid::Uuid;

/// Runtime configuration for the node service.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub build_id: String,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
        let build_id =
            env::var("CRYPTOCHAT_BUILD_ID").unwrap_or_else(|_| Uuid::new_v4().to_string());
        Ok(Self {
            host,
            port,
            build_id,
        })
    }
}
