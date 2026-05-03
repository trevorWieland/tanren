//! Shared `{code, summary}` error body and `AppServiceError` mapping.
//!
//! Split out of `lib.rs` so the api-app crate stays under the workspace
//! 500-line line-budget. The shapes here are the wire equivalent of
//! `tanren_contract::AccountFailureReason` rendered through the
//! API's HTTP transport.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tanren_app_services::AppServiceError;
use tanren_contract::AccountFailureReason;

/// Shared `{code, summary}` failure body.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AccountFailureBody {
    /// Stable error code from the closed taxonomy.
    pub code: String,
    /// Human-readable summary.
    pub summary: String,
}

/// Render the standard `internal_error` body for failed cookie-session
/// writes. Shared between the sign-up / sign-in / accept-invitation
/// routes.
pub(crate) fn session_install_error(err: &anyhow::Error) -> Response {
    tracing::error!(target: "tanren_api", error = %err, "session install");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AccountFailureBody {
            code: "internal_error".to_owned(),
            summary: "Tanren encountered an internal error.".to_owned(),
        }),
    )
        .into_response()
}

/// Map an [`AppServiceError`] to the matching HTTP response.
pub(crate) fn map_app_error(err: AppServiceError) -> Response {
    match err {
        AppServiceError::Account(reason) => failure_body(reason),
        AppServiceError::InvalidInput(message) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"code": "validation_failed", "summary": message})),
        )
            .into_response(),
        AppServiceError::Store(err) => {
            tracing::error!(target: "tanren_api", error = %err, "store error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "code": "internal_error",
                    "summary": "Tanren encountered an internal error.",
                })),
            )
                .into_response()
        }
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "code": "internal_error",
                "summary": "Tanren encountered an internal error.",
            })),
        )
            .into_response(),
    }
}

fn failure_body(reason: AccountFailureReason) -> Response {
    let status =
        StatusCode::from_u16(reason.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(json!({"code": reason.code(), "summary": reason.summary()})),
    )
        .into_response()
}
