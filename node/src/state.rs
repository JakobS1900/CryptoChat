use crate::config::AppConfig;
use std::sync::Arc;

#[derive(Debug)]
pub struct AppState {
    config: AppConfig,
}

impl AppState {
    pub fn new(config: AppConfig) -> Arc<Self> {
        Arc::new(Self { config })
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn build_id(&self) -> &str {
        &self.config.build_id
    }
}
