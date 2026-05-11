//! Shared `{code, summary}` error body and `AppServiceError` mapping.
//!
//! Split out of `lib.rs` so the api-app crate stays under the workspace
//! 500-line line-budget. The body types live in `tanren_contract::failure`;
//! this module adds HTTP-response mapping helpers.

use axum::Json;
use axum::extract::{FromRequest, Request, rejection::JsonRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AccountFailureBody, AccountFailureReason, ProjectFailureBody, ProjectFailureReason,
};

/// Render the standard `internal_error` body for failed cookie-session
/// writes. Shared between the sign-up / sign-in / accept-invitation
/// routes.
pub(crate) fn session_install_error(err: &anyhow::Error) -> Response {
    tracing::error!(target: "tanren_api", error = %err, "session install");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AccountFailureBody::internal_error()),
    )
        .into_response()
}

/// Render the canonical authentication-required failure body.
pub(crate) fn auth_required() -> Response {
    let reason = ProjectFailureReason::AuthRequired;
    (
        StatusCode::from_u16(reason.http_status()).unwrap_or(StatusCode::UNAUTHORIZED),
        Json(ProjectFailureBody::from_reason(reason)),
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
            Json(AccountFailureBody::validation_failed(message)),
        )
            .into_response(),
        AppServiceError::Store(err) => {
            tracing::error!(target: "tanren_api", error = %err, "store error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AccountFailureBody::internal_error()),
            )
                .into_response()
        }
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AccountFailureBody::internal_error()),
        )
            .into_response(),
    }
}

fn failure_body(reason: AccountFailureReason) -> Response {
    let status =
        StatusCode::from_u16(reason.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(AccountFailureBody::from_reason(reason))).into_response()
}

fn project_failure_body(reason: ProjectFailureReason) -> Response {
    let status =
        StatusCode::from_u16(reason.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(ProjectFailureBody::from_reason(reason))).into_response()
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

/// Project-route JSON extractor that maps all deserialize failures to the
/// project failure taxonomy with a stable validation summary.
#[derive(Debug)]
pub(crate) struct ProjectValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(map_account_json_rejection(&rejection)),
        }
    }
}

impl<S, T> FromRequest<S> for ProjectValidatedJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(map_project_json_rejection(&rejection)),
        }
    }
}

fn rejection_summary(rejection: &JsonRejection) -> String {
    match rejection {
        JsonRejection::MissingJsonContentType(_) => {
            "request body must be application/json".to_owned()
        }
        other => other.body_text(),
    }
}

fn map_account_json_rejection(rejection: &JsonRejection) -> Response {
    let summary = rejection_summary(rejection);
    (
        StatusCode::BAD_REQUEST,
        Json(AccountFailureBody::validation_failed(summary)),
    )
        .into_response()
}

fn map_project_json_rejection(rejection: &JsonRejection) -> Response {
    let reason = ProjectFailureReason::ValidationFailed;
    let summary = match rejection {
        JsonRejection::MissingJsonContentType(_) => {
            "The project request body must use application/json.".to_owned()
        }
        _ => reason.summary().to_owned(),
    };
    (
        StatusCode::BAD_REQUEST,
        Json(ProjectFailureBody::validation_failed(summary)),
    )
        .into_response()
}
