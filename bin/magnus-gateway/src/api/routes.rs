//! Route handlers for the Banking Gateway REST API.

use axum::{
    Json,
    extract::{Path, State, WebSocketUpgrade, ws::WebSocket},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    iso20022::{builder, parser},
    storage::local::{AuditStore, PaymentRecord},
};

/// Shared application state accessible by all route handlers.
pub struct AppState {
    /// SQLite audit store.
    pub audit: AuditStore,
}

// ---------------------------------------------------------------------------
// POST /api/v1/payments/initiate
// ---------------------------------------------------------------------------

/// Request body for payment initiation (raw pain.001 XML).
#[derive(Debug, Deserialize)]
pub struct InitiateRequest {
    /// Raw pain.001 XML content.
    pub xml: String,
}

/// Response after successful payment initiation.
#[derive(Debug, Serialize, Deserialize)]
pub struct InitiateResponse {
    /// End-to-end ID extracted from pain.001.
    pub end_to_end_id: String,
    /// Audit record ID.
    pub record_id: i64,
    /// Current status.
    pub status: String,
}

/// Accept a pain.001 message, parse it, and record in the audit store.
pub async fn initiate_payment(
    State(state): State<Arc<AppState>>,
    Json(body): Json<InitiateRequest>,
) -> Result<Json<InitiateResponse>, (StatusCode, String)> {
    // Parse the pain.001 XML
    let instruction = parser::parse_pain001(&body.xml)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid pain.001: {e}")))?;

    if instruction.transactions.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No credit transfers in message".into()));
    }

    let transfer = &instruction.transactions[0];
    let end_to_end_id = transfer.end_to_end_id.clone();
    let debtor = &transfer.debtor.name;

    // Insert into audit store
    let record_id = state
        .audit
        .insert_payment(&end_to_end_id, debtor)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}")))?;

    Ok(Json(InitiateResponse {
        end_to_end_id,
        record_id,
        status: "received".to_string(),
    }))
}

// ---------------------------------------------------------------------------
// GET /api/v1/payments/:end_to_end_id
// ---------------------------------------------------------------------------

/// Response for a payment status query.
#[derive(Debug, Serialize)]
pub struct PaymentStatusResponse {
    #[serde(flatten)]
    pub record: PaymentRecord,
}

/// Query the status of a payment by its end-to-end ID.
pub async fn get_payment_status(
    State(state): State<Arc<AppState>>,
    Path(end_to_end_id): Path<String>,
) -> Result<Json<PaymentStatusResponse>, StatusCode> {
    let record = state
        .audit
        .query_by_end_to_end_id(&end_to_end_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(PaymentStatusResponse { record }))
}

// ---------------------------------------------------------------------------
// GET /api/v1/statements/:account
// ---------------------------------------------------------------------------

/// Response containing a generated camt.053 bank-to-customer statement.
#[derive(Debug, Serialize)]
pub struct StatementResponse {
    /// camt.053 XML content.
    pub xml: String,
    /// Number of entries in the statement.
    pub entry_count: usize,
}

/// Generate a camt.053 statement for an account.
pub async fn get_statement(
    State(state): State<Arc<AppState>>,
    Path(account): Path<String>,
) -> Result<Json<StatementResponse>, StatusCode> {
    let records = state
        .audit
        .query_by_account(&account)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Convert payment records into statement entries
    let entries: Vec<crate::iso20022::types::StatementEntry> = records
        .iter()
        .map(|r| crate::iso20022::types::StatementEntry {
            entry_ref: r.end_to_end_id.clone(),
            end_to_end_id: r.end_to_end_id.clone(),
            amount: crate::iso20022::types::Amount {
                value: "0.00".to_string(),
                currency: "USD".to_string(),
            },
            cd_indicator: "CRDT".to_string(),
            booking_date: r.created_at.clone(),
            value_date: r.created_at.clone(),
            remittance_info: None,
        })
        .collect();

    let entry_count = entries.len();
    let stmt_id = format!("STMT-{}", &account[..8.min(account.len())]);
    let now = chrono::Utc::now().to_rfc3339();
    let xml = builder::build_camt053(&stmt_id, &account, &entries, &now);

    Ok(Json(StatementResponse { xml, entry_count }))
}

// ---------------------------------------------------------------------------
// WS /api/v1/notifications
// ---------------------------------------------------------------------------

/// Upgrade to WebSocket for push notifications (camt.054 events).
pub async fn ws_notifications(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_ws)
}

/// Handle an active WebSocket connection.
async fn handle_ws(mut socket: WebSocket) {
    // In production, this would subscribe to a broadcast channel
    // and push camt.054 XML messages to the client as events occur.
    //
    // For MVP, we send a welcome message and keep alive.
    use axum::extract::ws::Message;

    let welcome = serde_json::json!({
        "type": "connected",
        "message": "Magnus Gateway notification stream"
    });

    if socket
        .send(Message::Text(welcome.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    // Keep connection alive, echo back any received messages
    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Text(text) => {
                if socket.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// GET /api/v1/health
// ---------------------------------------------------------------------------

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Service status.
    pub status: String,
    /// Service version.
    pub version: String,
}

/// Health check endpoint.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::{get, post},
    };
    use tower::ServiceExt;

    fn test_app() -> Router {
        let store = AuditStore::open_memory().unwrap();
        let state = Arc::new(AppState { audit: store });

        Router::new()
            .route("/health", get(health))
            .route("/payments/initiate", post(initiate_payment))
            .route("/payments/:end_to_end_id", get(get_payment_status))
            .route("/statements/:account", get(get_statement))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_initiate_and_query() {
        let store = AuditStore::open_memory().unwrap();
        let state = Arc::new(AppState { audit: store });

        let app = Router::new()
            .route("/payments/initiate", post(initiate_payment))
            .route("/payments/:end_to_end_id", get(get_payment_status))
            .with_state(state);

        // Initiate a payment with pain.001 XML
        let pain001 = r#"<?xml version="1.0"?>
<Document>
  <CstmrCdtTrfInitn>
    <GrpHdr>
      <MsgId>MSG-001</MsgId>
      <CreDtTm>2025-01-15T10:00:00</CreDtTm>
      <NbOfTxs>1</NbOfTxs>
    </GrpHdr>
    <PmtInf>
      <PmtInfId>PMT-001</PmtInfId>
      <PmtMtd>TRF</PmtMtd>
      <Dbtr><Nm>Alice Corp</Nm></Dbtr>
      <CdtTrfTxInf>
        <PmtId><EndToEndId>E2E-TEST-001</EndToEndId></PmtId>
        <Amt><InstdAmt Ccy="USD">1000.00</InstdAmt></Amt>
        <Cdtr><Nm>Bob Inc</Nm></Cdtr>
      </CdtTrfTxInf>
    </PmtInf>
  </CstmrCdtTrfInitn>
</Document>"#;

        let body = serde_json::json!({ "xml": pain001 });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/payments/initiate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: InitiateResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp.end_to_end_id, "E2E-TEST-001");
        assert_eq!(resp.status, "received");

        // Query the payment status
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/payments/E2E-TEST-001")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_payment_not_found() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/payments/DOES-NOT-EXIST")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
