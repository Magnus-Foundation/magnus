//! API key authentication middleware.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

/// Validate the `X-API-Key` header against the configured key.
pub async fn api_key_auth(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // The API key is stored in request extensions by the server layer.
    let expected = request
        .extensions()
        .get::<ApiKey>()
        .map(|k| k.0.clone());

    let provided = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    match (expected, provided) {
        (Some(expected), Some(provided)) if expected == provided => Ok(next.run(request).await),
        (None, _) => {
            // No key configured — allow (dev mode)
            Ok(next.run(request).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Wrapper type for the API key stored in request extensions.
#[derive(Clone)]
pub struct ApiKey(pub String);
