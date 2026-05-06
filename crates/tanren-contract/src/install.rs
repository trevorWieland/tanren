//! Install drift command/response wire shapes.
//!
//! These types are the request/response surface used by interface binaries
//! when callers request a read-only drift check against an installed
//! repository. They live in `tanren-contract` because every interface
//! serialises the same shapes — keeping them here is the architectural
//! guarantee that the surfaces stay equivalent.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::ProjectId;
use utoipa::ToSchema;

/// Kind of install asset being checked for drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstallDriftAssetKind {
    /// A generated asset whose content must match a fresh install exactly.
    Generated,
    /// A preserved standard that users may edit without it being reported
    /// as drift.
    PreservedStandard,
}

/// Drift state of a single install asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstallDriftState {
    /// The asset matches what a fresh install would produce.
    Matches,
    /// The asset's content differs from a fresh install.
    Drifted,
    /// The asset is missing from the repository.
    Missing,
    /// A preserved standard with user edits, accepted as non-drift.
    Accepted,
}

/// Policy governing how preserved standards are evaluated for drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreservationPolicy {
    /// User edits to preserved standards are accepted and not reported as
    /// drift.
    AcceptUserEdits,
    /// Any deviation from the fresh install content is reported as drift,
    /// including user edits to preserved standards.
    Strict,
}

/// Policy governing what the drift check considers reportable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DriftPolicy {
    /// Report drift for generated assets only.
    GeneratedOnly,
    /// Report drift for both generated assets and preserved standards.
    AllAssets,
}

/// Request to check an installed repository for drift.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InstallDriftRequest {
    /// Project to check for drift.
    pub project_id: ProjectId,
}

/// Single entry in a drift report, describing one asset's state.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InstallDriftEntry {
    /// Path of the asset relative to the repository root.
    pub relative_path: String,
    /// Kind of the asset.
    pub asset_kind: InstallDriftAssetKind,
    /// Drift state of the asset.
    pub state: InstallDriftState,
}

/// Describes the effective drift configuration applied during the check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DriftConfigSource {
    /// The drift policy that was applied.
    pub drift_policy: DriftPolicy,
    /// The preservation policy that was applied.
    pub preservation_policy: PreservationPolicy,
}

/// Response from an install drift check.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InstallDriftResponse {
    /// Whether any drift was detected.
    pub has_drift: bool,
    /// Entries for every asset checked.
    pub entries: Vec<InstallDriftEntry>,
    /// Description of the effective drift configuration that was applied.
    pub config_source: DriftConfigSource,
}
