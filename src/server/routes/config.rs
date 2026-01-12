//! Configuration endpoint

use std::sync::Arc;

use axum::{extract::State, Json};

use crate::server::state::AppState;
use crate::server::types::ConfigResponse;

/// GET /api/config
pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<ConfigResponse> {
    let config = state.config.read().await;

    Json(ConfigResponse {
        provider: format!("{:?}", config.llm.provider),
        model: config.llm.model.clone(),
        mode: "build".to_string(), // Default mode
    })
}
