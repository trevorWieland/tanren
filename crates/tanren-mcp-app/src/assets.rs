//! MCP upgrade tool parameter types and error mapping.
//!
//! Split out of `lib.rs` to keep the main module under the workspace
//! 500-line line-budget. The `#[rmcp::tool]` method bodies in `lib.rs`
//! delegate to helpers here.

use rmcp::model::{CallToolResult, Content};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tanren_app_services::{ApplyError, PreviewError};

/// Parameters for the `upgrade.preview` MCP tool.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct UpgradePreviewParams {
    /// Absolute or relative path to the repository root.
    pub(crate) root: String,
}

/// Parameters for the `upgrade.apply` MCP tool.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct UpgradeApplyParams {
    /// Absolute or relative path to the repository root.
    pub(crate) root: String,
    /// Must be `true` to confirm the upgrade. Returns
    /// `confirmation_required` when absent or false.
    pub(crate) confirm: bool,
}

/// Map a [`PreviewError`] to an MCP tool failure result.
pub(crate) fn map_preview_failure(err: &PreviewError) -> CallToolResult {
    let (code, summary) = match err {
        PreviewError::RootNotFound(path) => (
            "root_not_found",
            format!("Root directory does not exist: {}", path.display()),
        ),
        PreviewError::ManifestMissing(path) => (
            "manifest_missing",
            format!("Asset manifest not found at {}", path.display()),
        ),
        PreviewError::ManifestParse(msg) => (
            "manifest_parse_error",
            format!("Failed to parse asset manifest: {msg}"),
        ),
        PreviewError::UnsupportedVersion {
            manifest,
            supported,
        } => (
            "unsupported_manifest_version",
            format!("Manifest version {manifest} is unsupported (supported: {supported})"),
        ),
        _ => ("internal_error", format!("Upgrade preview failed: {err}")),
    };
    failure_text(code, &summary)
}

/// Map an [`ApplyError`] to an MCP tool failure result.
pub(crate) fn map_apply_failure(err: ApplyError) -> CallToolResult {
    let (code, summary) = match err {
        ApplyError::Preview(preview_err) => return map_preview_failure(&preview_err),
        ApplyError::UnreportedDrift {
            path,
            recorded,
            observed,
        } => (
            "unreported_drift",
            format!(
                "Drift detected for {}: on-disk hash {} differs from manifest hash {}",
                path.display(),
                observed,
                recorded
            ),
        ),
        ApplyError::Io { path, source } => {
            tracing::error!(target: "tanren_mcp", path = %path.display(), error = %source, "upgrade I/O error");
            (
                "internal_error",
                "Tanren encountered an internal error.".to_owned(),
            )
        }
        ApplyError::ManifestWrite(msg) => {
            tracing::error!(target: "tanren_mcp", error = %msg, "manifest write error");
            (
                "internal_error",
                "Tanren encountered an internal error.".to_owned(),
            )
        }
        _ => (
            "internal_error",
            "Tanren encountered an internal error.".to_owned(),
        ),
    };
    failure_text(code, &summary)
}

/// Build a `confirmation_required` failure result.
pub(crate) fn confirmation_required() -> CallToolResult {
    failure_text(
        "confirmation_required",
        "Set confirm to true to apply the upgrade.",
    )
}

fn failure_text(code: &str, summary: &str) -> CallToolResult {
    let body = json!({
        "code": code,
        "summary": summary,
    });
    let text = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_owned());
    CallToolResult::error(vec![Content::text(text)])
}
