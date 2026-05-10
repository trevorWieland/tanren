//! Standards inspect report wire-shape contracts.
//!
//! These types are the `standards.inspect` command's success report surface
//! used by the CLI, API, MCP, TUI, and web client. They live in
//! `tanren-contract` so every interface binary serialises the same shapes —
//! keeping them here is the architectural guarantee that the surfaces stay
//! equivalent.

use serde::{Deserialize, Serialize};

/// Effective-configuration setting family wire representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationSettingFamily {
    /// Standards methodology profile adoption.
    StandardsProfile,
    /// Standards repository root selection.
    StandardsRoot,
}

/// Configuration source scope wire representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationSourceScope {
    /// User-scoped configuration.
    User,
    /// Account-scoped configuration.
    Account,
    /// Organization-scoped configuration.
    Organization,
    /// Project-scoped configuration.
    Project,
    /// Service-account scoped configuration.
    ServiceAccount,
    /// Assignment-scoped configuration.
    Assignment,
    /// Installation-scoped configuration.
    Installation,
}

/// Resolution shape wire representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationResolutionKind {
    /// Declared directly at the source scope.
    Explicit,
    /// Inherited from an upstream scope.
    Inherited,
    /// Applied from a default.
    Defaulted,
    /// Overridden by a more specific setting.
    Overridden,
    /// Locked by policy.
    Locked,
}

/// Policy constraint wire representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationPolicyConstraint {
    /// No additional constraint applied.
    None,
}

/// Actor usability wire representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationActorUsability {
    /// Usable by the current actor.
    Usable,
    /// Not usable by the current actor.
    NotUsable,
}

/// Freshness state wire representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfigurationFreshness {
    /// Projection is current for the inspected repository snapshot.
    Current,
    /// Projection may be stale.
    Stale,
}

/// Resolution metadata wire shape attached to an effective configuration field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveConfigurationMetadataWire {
    /// Setting family associated with this resolved field.
    pub setting_family: EffectiveConfigurationSettingFamily,
    /// Scope where the effective value originated.
    pub source_scope: EffectiveConfigurationSourceScope,
    /// How the value became effective.
    pub resolution_kind: EffectiveConfigurationResolutionKind,
    /// Policy constraint applied during resolution.
    pub policy_constraint: EffectiveConfigurationPolicyConstraint,
    /// Whether the value is usable by the active actor.
    pub actor_usability: EffectiveConfigurationActorUsability,
    /// Freshness state of the projection that produced this value.
    pub freshness: EffectiveConfigurationFreshness,
    /// Projection position, when available.
    pub projection_position: Option<u64>,
}

impl EffectiveConfigurationMetadataWire {
    /// Create project-scoped explicit metadata for a standards setting.
    #[must_use]
    pub const fn project_explicit(setting_family: EffectiveConfigurationSettingFamily) -> Self {
        Self {
            setting_family,
            source_scope: EffectiveConfigurationSourceScope::Project,
            resolution_kind: EffectiveConfigurationResolutionKind::Explicit,
            policy_constraint: EffectiveConfigurationPolicyConstraint::None,
            actor_usability: EffectiveConfigurationActorUsability::Usable,
            freshness: EffectiveConfigurationFreshness::Current,
            projection_position: None,
        }
    }
}

/// Effective-configuration section of a standards inspect success report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct StandardsInspectEffectiveConfigurationReport {
    /// Effective-configuration metadata for the standards profile.
    pub profile: EffectiveConfigurationMetadataWire,
    /// Effective-configuration metadata for the standards root.
    pub standards_root: EffectiveConfigurationMetadataWire,
}

/// Status of a standards inspect success report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandardsInspectReportStatus {
    /// Inspection succeeded.
    Ok,
}

/// Command identifier within a standards inspect report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StandardsInspectReportCommand {
    /// The `standards.inspect` command.
    #[serde(rename = "standards.inspect")]
    StandardsInspect,
}

/// `standards.inspect` success report wire shape.
///
/// Serialised as a single JSON object to stdout by the CLI and returned
/// as a JSON body by the API/MCP surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct StandardsInspectReport {
    /// Report status.
    pub status: StandardsInspectReportStatus,
    /// Command that produced this report.
    pub command: StandardsInspectReportCommand,
    /// Repository argument (repo-relative or redacted).
    pub repository: String,
    /// Methodology profile name.
    pub profile: String,
    /// Repository-relative standards root path.
    pub standards_root: String,
    /// Count of standards files found.
    pub standards_count: usize,
    /// Name of the first standard (sorted by path).
    pub first_standard_name: String,
    /// Repo-relative path of the first standard.
    pub first_standard_path: String,
    /// Effective-configuration resolution metadata.
    pub effective_configuration: StandardsInspectEffectiveConfigurationReport,
}
