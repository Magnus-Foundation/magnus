//! Axum HTTP server for the Banking Gateway API.

use std::sync::Arc;

use axum::{
    Router,
    middleware,
    routing::{any, get, post},
};
use eyre::Result;
use tower_http::cors::CorsLayer;

use crate::config::GatewayConfig;

use super::{
    auth::{ApiKey, api_key_auth},
    routes::{self, AppState},
};

use crate::storage::local::AuditStore;

/// Start the API server with the given configuration.
pub async fn run(config: GatewayConfig) -> Result<()> {
    let db_path = "magnus_gateway_audit.db";
    let store = AuditStore::open(db_path)?;
    let state = Arc::new(AppState { audit: store });

    let api_key = ApiKey(config.api.api_key.clone());

    let app = Router::new()
        .route("/api/v1/health", get(routes::health))
        .route("/api/v1/payments/initiate", post(routes::initiate_payment))
        .route(
            "/api/v1/payments/:end_to_end_id",
            get(routes::get_payment_status),
        )
        .route(
            "/api/v1/statements/:account",
            get(routes::get_statement),
        )
        .route(
            "/api/v1/notifications",
            any(routes::ws_notifications),
        )
        .layer(middleware::from_fn(api_key_auth))
        .layer(axum::Extension(api_key))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.api.bind).await?;
    tracing::info!(bind = %config.api.bind, "API server listening");

    axum::serve(listener, app).await?;

    Ok(())
}
