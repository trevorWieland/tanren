//! Shared `{code, summary}` error body and `AppServiceError` mapping.
//!
//! Split out of `lib.rs` so the api-app crate stays under the workspace
//! 500-line line-budget. The shapes here are the wire equivalent of
//! `tanren_contract::AccountFailureReason` rendered through the
//! API's HTTP transport.

use axum::Json;
use axum::extract::{FromRequest, Request, rejection::JsonRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tanren_app_services::AppServiceError;
use tanren_contract::{AccountFailureReason, ProjectFailureReason};
use tanren_store::StoreError;

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
        AppServiceError::Project(reason) => project_failure_body(reason),
        AppServiceError::InvalidInput(message) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"code": "validation_failed", "summary": message})),
        )
            .into_response(),
        AppServiceError::Store(StoreError::UnauthorizedProjectAccess) => {
            project_failure_body(ProjectFailureReason::UnauthorizedProjectAccess)
        }
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

fn project_failure_body(reason: ProjectFailureReason) -> Response {
    let status =
        StatusCode::from_u16(reason.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(AccountFailureBody {
            code: reason.code().to_owned(),
            summary: reason.summary().to_owned(),
        }),
    )
        .into_response()
}

/// Render the shared `unauthenticated` failure body for routes that
/// require a cookie session but received none (or an expired one).
pub(crate) fn unauthenticated_error() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(AccountFailureBody {
            code: "unauthenticated".to_owned(),
            summary: "Authentication is required.".to_owned(),
        }),
    )
        .into_response()
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

/// Custom `Json` extractor that maps any deserialize-time failure
/// (malformed JSON, missing required field, OR a validating-newtype
/// `Deserialize` impl returning an error — e.g. `Email::parse` rejecting
/// an RFC-malformed address) to the shared `{code, summary}` taxonomy:
/// `400 Bad Request` with `code = "validation_failed"`. Without this
/// wrapper, axum's default behaviour returns 422 with a plain-text body
/// that bypasses the wire taxonomy clients depend on, and the new
/// validating-`Deserialize` impls on `Email` / `Identifier` /
/// `InvitationToken` (Codex P1 review on PR #133) would surface as
/// untyped 422s.
#[derive(Debug)]
pub(crate) struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(map_json_rejection(&rejection)),
        }
    }
}

fn map_json_rejection(rejection: &JsonRejection) -> Response {
    let summary = match rejection {
        JsonRejection::JsonDataError(e) => e.body_text(),
        JsonRejection::JsonSyntaxError(e) => e.body_text(),
        JsonRejection::MissingJsonContentType(_) => {
            "request body must be application/json".to_owned()
        }
        other => other.body_text(),
    };
    (
        StatusCode::BAD_REQUEST,
        Json(AccountFailureBody {
            code: "validation_failed".to_owned(),
            summary,
        }),
    )
        .into_response()
}
