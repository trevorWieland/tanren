//! Asset upgrade contract shapes.
//!
//! Types for the versioned asset manifest, diff actions, migration concerns,
//! upgrade request/response bodies, and failure DTOs. Shared across all
//! Tanren interface surfaces (CLI, API, MCP, TUI).

use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Current format version of the asset manifest file.
pub const MANIFEST_FORMAT_VERSION: u32 = 1;

/// Versioned asset manifest — the `.tanren/asset-manifest` file that records
/// every asset Tanren installed into a repository.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct AssetManifest {
    /// Format version of the manifest file itself.
    pub version: u32,
    /// Tanren version that produced this manifest.
    pub source_version: String,
    /// Assets recorded in this manifest.
    #[serde(default)]
    pub assets: Vec<AssetEntry>,
}

/// A single asset recorded in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct AssetEntry {
    /// Path relative to the repository root.
    #[schemars(with = "String")]
    #[schema(value_type = String)]
    pub path: PathBuf,
    /// Content hash in the form `sha256:<hex>`.
    pub hash: String,
    /// Whether Tanren or the user owns this asset.
    pub ownership: AssetOwnership,
    /// Tanren version that installed (or last updated) this asset.
    pub installed_from: String,
}

/// Asset ownership discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetOwnership {
    /// Tanren-generated; the upgrade planner may propose changes.
    Tanren,
    /// User-owned; always preserved by the upgrade planner.
    User,
}

/// Planned action for a single asset during an upgrade preview.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AssetAction {
    /// Asset does not exist in the installed manifest; will be created.
    Create {
        /// Relative path of the new asset.
        #[schemars(with = "String")]
        #[schema(value_type = String)]
        path: PathBuf,
        /// Hash of the content that will be written.
        hash: String,
    },
    /// Asset exists but content differs; will be updated.
    Update {
        /// Relative path of the updated asset.
        #[schemars(with = "String")]
        #[schema(value_type = String)]
        path: PathBuf,
        /// Hash of the currently installed content.
        old_hash: String,
        /// Hash of the content that will replace it.
        new_hash: String,
    },
    /// Asset exists in the installed manifest but not in the target bundle;
    /// will be removed.
    Remove {
        /// Relative path of the removed asset.
        #[schemars(with = "String")]
        #[schema(value_type = String)]
        path: PathBuf,
        /// Hash of the content that will be removed.
        old_hash: String,
    },
    /// User-owned asset; will not be modified.
    Preserve {
        /// Relative path of the preserved asset.
        #[schemars(with = "String")]
        #[schema(value_type = String)]
        path: PathBuf,
        /// Current content hash.
        hash: String,
    },
}

impl AssetAction {
    /// The relative path this action refers to.
    #[must_use]
    pub fn path(&self) -> &PathBuf {
        match self {
            Self::Create { path, .. }
            | Self::Update { path, .. }
            | Self::Remove { path, .. }
            | Self::Preserve { path, .. } => path,
        }
    }
}

/// A migration concern flagged during preview — not a blocker, but something
/// the user should review before applying the upgrade.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MigrationConcern {
    /// Category of concern.
    pub kind: MigrationConcernKind,
    /// Relative path this concern relates to.
    #[schemars(with = "String")]
    #[schema(value_type = String)]
    pub path: PathBuf,
    /// Human-readable description of the concern.
    pub detail: String,
}

/// Closed taxonomy of migration concern categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MigrationConcernKind {
    /// A Tanren-generated asset was modified outside Tanren; the update
    /// will overwrite local changes.
    HashMismatch,
    /// An asset is being removed in the target version.
    RemovedAsset,
    /// The manifest version is older than the minimum supported version;
    /// a migration path exists but the user should verify.
    LegacyManifest,
    /// A user-owned asset would be affected by structural changes.
    UserAssetPathConflict,
}

/// Response shape returned by the upgrade preview planner.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpgradePreviewResponse {
    /// Version recorded in the installed manifest (source).
    pub source_version: String,
    /// Version of the embedded asset bundle (target).
    pub target_version: String,
    /// Planned actions for every asset.
    pub actions: Vec<AssetAction>,
    /// Migration concerns the user should review.
    pub concerns: Vec<MigrationConcern>,
    /// User-owned paths that will be preserved (no modification).
    #[schemars(with = "Vec<String>")]
    #[schema(value_type = Vec<String>)]
    pub preserved_user_paths: Vec<PathBuf>,
}

/// Request body for the upgrade preview operation.
///
/// Identifies the target installation by identity rather than by filesystem
/// path, so the contract remains stable across host machines and CI runners.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpgradePreviewRequest {
    /// Installation identifier for the Tanren-managed project.
    pub installation_id: String,
}

/// Request body for the confirmed upgrade apply operation.
///
/// Carries an explicit confirmation flag and a client-provided idempotency
/// key so that retries of the same logical operation are deduplicated by
/// the handler.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpgradeApplyRequest {
    /// Installation identifier for the Tanren-managed project.
    pub installation_id: String,
    /// Must be `true` to confirm the upgrade. The handler returns
    /// `confirmation_required` when absent or false.
    pub confirm: bool,
    /// Client-provided idempotency key. The handler deduplicates apply
    /// operations carrying the same key.
    pub operation_id: String,
}

/// Shared failure body for asset upgrade operations.
///
/// Maps onto the same `{code, summary}` wire taxonomy used by account-flow
/// errors but lives in `tanren-contract` so API, MCP, CLI, and TUI adapters
/// can project the same shape without importing interface-specific crates.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpgradeFailureBody {
    /// Stable error code from the closed upgrade-failure taxonomy.
    pub code: String,
    /// Human-readable summary.
    pub summary: String,
}
