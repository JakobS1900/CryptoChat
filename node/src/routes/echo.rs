use crate::state::AppState;
use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

#[derive(Debug, Deserialize, Serialize)]
pub struct EchoPayload {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct EchoResponse {
    pub echoed: String,
    pub build_id: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/echo", post(echo))
}

async fn echo(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EchoPayload>,
) -> impl IntoResponse {
    debug!(message = %payload.message, "echo request");
    Json(EchoResponse {
        echoed: payload.message,
        build_id: state.build_id().to_string(),
    })
}
