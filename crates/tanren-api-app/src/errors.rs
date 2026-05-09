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
use tanren_app_services::AppServiceError;
use tanren_contract::{AccountFailureReason, InterfaceError, InterfaceErrorCode};

/// Render the standard `internal_error` body for failed cookie-session
/// writes. Shared between the sign-up / sign-in / accept-invitation
/// routes.
pub(crate) fn session_install_error(err: &anyhow::Error) -> Response {
    tracing::error!(target: "tanren_api", error = %err, "session install");
    internal_error_response().into_response()
}

/// Shared `401 auth_required` error body.
pub(crate) fn auth_required_response(summary: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(InterfaceError::new(
            InterfaceErrorCode::AuthRequired,
            summary,
        )),
    )
        .into_response()
}

/// Shared `500 internal_error` response body.
pub(crate) fn internal_error_response() -> (StatusCode, Json<InterfaceError>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(InterfaceError::new(
            InterfaceErrorCode::InternalError,
            "Tanren encountered an internal error.",
        )),
    )
}

/// Map an [`AppServiceError`] to the matching HTTP response.
pub(crate) fn map_app_error(err: AppServiceError) -> Response {
    match err {
        AppServiceError::Account(reason) => failure_body(reason),
        AppServiceError::Permissions(reason) => (
            StatusCode::FORBIDDEN,
            Json(InterfaceError::new(
                reason.interface_error_code(),
                reason.summary(),
            )),
        )
            .into_response(),
        AppServiceError::InvalidInput(message) => (
            StatusCode::BAD_REQUEST,
            Json(InterfaceError::new(
                InterfaceErrorCode::ValidationFailed,
                message,
            )),
        )
            .into_response(),
        AppServiceError::Store(err) => {
            tracing::error!(target: "tanren_api", error = %err, "store error");
            internal_error_response().into_response()
        }
        _ => internal_error_response().into_response(),
    }
}

fn failure_body(reason: AccountFailureReason) -> Response {
    let status =
        StatusCode::from_u16(reason.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(InterfaceError::new(
            reason.interface_error_code(),
            reason.summary(),
        )),
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
        Json(InterfaceError::new(
            InterfaceErrorCode::ValidationFailed,
            summary,
        )),
    )
        .into_response()
}
