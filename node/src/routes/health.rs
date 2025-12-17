use crate::state::AppState;
use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: &'static str,
    build_id: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health_check))
}

async fn health_check(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        build_id: state.build_id().to_string(),
    })
}
