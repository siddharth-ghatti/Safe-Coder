//! Health check endpoint

use axum::Json;

use crate::server::types::HealthResponse;

/// GET /api/health
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
