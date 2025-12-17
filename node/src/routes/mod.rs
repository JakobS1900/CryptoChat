pub mod echo;
pub mod health;

use crate::state::AppState;
use axum::Router;
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(health::routes())
        .merge(echo::routes())
        .with_state(state)
}
