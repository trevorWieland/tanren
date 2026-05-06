//! Asset upgrade error mapping.
//!
//! Split out of `routes.rs` so each file stays under the workspace 500-line
//! line-budget. The route handlers live in `routes.rs`; the error mapping
//! lives here. Request and failure DTOs are imported from `tanren-contract`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tanren_app_services::{ApplyError, PreviewError};
use tanren_contract::UpgradeFailureBody;

/// Map a [`PreviewError`] to an HTTP response.
pub(crate) fn map_preview_error(err: &PreviewError) -> Response {
    let (status, code, summary) = match err {
        PreviewError::RootNotFound(path) => (
            StatusCode::NOT_FOUND,
            "root_not_found",
            format!("Root directory does not exist: {}", path.display()),
        ),
        PreviewError::ManifestMissing(path) => (
            StatusCode::NOT_FOUND,
            "manifest_missing",
            format!("Asset manifest not found at {}", path.display()),
        ),
        PreviewError::ManifestParse(msg) => (
            StatusCode::BAD_REQUEST,
            "manifest_parse_error",
            format!("Failed to parse asset manifest: {msg}"),
        ),
        PreviewError::UnsupportedVersion {
            manifest,
            supported,
        } => (
            StatusCode::BAD_REQUEST,
            "unsupported_manifest_version",
            format!("Manifest version {manifest} is unsupported (supported: {supported})"),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            format!("Upgrade preview failed: {err}"),
        ),
    };
    (
        status,
        Json(UpgradeFailureBody {
            code: code.to_owned(),
            summary,
        }),
    )
        .into_response()
}

/// Map an [`ApplyError`] to an HTTP response.
pub(crate) fn map_apply_error(err: ApplyError) -> Response {
    match err {
        ApplyError::Preview(preview_err) => map_preview_error(&preview_err),
        ApplyError::UnreportedDrift {
            path,
            recorded,
            observed,
        } => (
            StatusCode::CONFLICT,
            Json(UpgradeFailureBody {
                code: "unreported_drift".to_owned(),
                summary: format!(
                    "Drift detected for {}: on-disk hash {} differs from manifest hash {}",
                    path.display(),
                    observed,
                    recorded
                ),
            }),
        )
            .into_response(),
        ApplyError::Io { path, source } => {
            tracing::error!(target: "tanren_api", path = %path.display(), error = %source, "upgrade I/O error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UpgradeFailureBody {
                    code: "internal_error".to_owned(),
                    summary: "Tanren encountered an internal error.".to_owned(),
                }),
            )
                .into_response()
        }
        ApplyError::ManifestWrite(msg) => {
            tracing::error!(target: "tanren_api", error = %msg, "manifest write error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UpgradeFailureBody {
                    code: "internal_error".to_owned(),
                    summary: "Tanren encountered an internal error.".to_owned(),
                }),
            )
                .into_response()
        }
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(UpgradeFailureBody {
                code: "internal_error".to_owned(),
                summary: "Tanren encountered an internal error.".to_owned(),
            }),
        )
            .into_response(),
    }
}
