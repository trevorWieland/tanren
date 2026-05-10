//! Payload size, path length, and label length limits for test-hook routes.
//!
//! Each limit is enforced before dispatch so oversized or malformed
//! requests receive a typed `BAD_REQUEST` response instead of reaching
//! the fixture state machine.

use axum::http::StatusCode;

/// Maximum JSON body size in bytes (64 KiB).
pub(super) const MAX_JSON_BODY_BYTES: usize = 64 * 1024;

/// Maximum fixture file content in bytes (1 MiB).
pub(super) const MAX_CONTENT_BYTES: usize = 1024 * 1024;

/// Maximum snapshot label length.
pub(super) const MAX_SNAPSHOT_LABEL_LEN: usize = 128;

/// Maximum repo-relative path length.
pub(super) const MAX_REPO_RELATIVE_PATH_LEN: usize = 512;

/// Validate JSON body byte size.
pub(super) fn validate_body_size(body: &serde_json::Value) -> Result<(), (StatusCode, String)> {
    let encoded_len = serde_json::to_vec(body).map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            format!("failed to measure payload size: {err}"),
        )
    })?;
    if encoded_len.len() > MAX_JSON_BODY_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("payload exceeds the {MAX_JSON_BODY_BYTES}-byte limit"),
        ));
    }
    Ok(())
}

/// Validate a fixture file content length.
pub(super) fn validate_content_len(content: &[u8]) -> Result<(), (StatusCode, String)> {
    if content.len() > MAX_CONTENT_BYTES {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "fixture content is {} bytes, exceeding the {}-byte limit",
                content.len(),
                MAX_CONTENT_BYTES
            ),
        ));
    }
    Ok(())
}

/// Validate a snapshot label.
pub(super) fn validate_snapshot_label(label: &str) -> Result<(), (StatusCode, String)> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "snapshot label must not be empty".to_owned(),
        ));
    }
    if trimmed.len() > MAX_SNAPSHOT_LABEL_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "snapshot label is {} bytes, exceeding the {}-byte limit",
                trimmed.len(),
                MAX_SNAPSHOT_LABEL_LEN
            ),
        ));
    }
    Ok(())
}

/// Maximum fixture/scenario id length.
pub(super) const MAX_FIXTURE_ID_LEN: usize = 128;

/// Validate a fixture/scenario id.
pub(super) fn validate_fixture_id(id: &str) -> Result<(), (StatusCode, String)> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "fixture_id must not be empty".to_owned(),
        ));
    }
    if trimmed.len() > MAX_FIXTURE_ID_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "fixture_id is {} bytes, exceeding the {}-byte limit",
                trimmed.len(),
                MAX_FIXTURE_ID_LEN
            ),
        ));
    }
    Ok(())
}

/// Validate a repo-relative path length.
pub(super) fn validate_repo_relative_path_len(path: &str) -> Result<(), (StatusCode, String)> {
    let trimmed = path.trim();
    if trimmed.len() > MAX_REPO_RELATIVE_PATH_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "repo-relative path is {} bytes, exceeding the {}-byte limit",
                trimmed.len(),
                MAX_REPO_RELATIVE_PATH_LEN
            ),
        ));
    }
    Ok(())
}
