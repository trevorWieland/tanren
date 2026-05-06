//! Install projection manifest — the authoritative catalog of Tanren-owned
//! generated projections and user-editable preserved standards.
//!
//! [`PROJECTION_MANIFEST`] contains only Tanren-owned generated assets.
//! [`PRESERVED_INPUTS`] contains user-editable preserved standards that are
//! presence-checked and reported separately. Both are static slices so no
//! heap allocation is required per access.

use tanren_contract::InstallDriftAssetKind;

/// Stable category identifying the kind of generated projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetCategory {
    /// A rendered command prompt projected into a harness destination.
    CommandProjection,
    /// Harness connection or configuration placeholder.
    HarnessConfig,
    /// Tanren metadata file.
    Metadata,
    /// MCP connection projection.
    McpConnection,
    /// API connection projection.
    ApiConnection,
}

/// Who owns the content of an installed asset after installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetOwnership {
    /// Tanren owns the content; any edit is drift.
    Tanren,
    /// User owns the content; edits are accepted as non-drift.
    User,
}

/// How drift is evaluated for a single manifest entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryDriftPolicy {
    /// Content must match the canonical source exactly; any difference is drift.
    ExactMatch,
    /// Only presence is checked; content edits are accepted (non-drift).
    PresenceOnly,
}

/// Describes a single Tanren-owned entry in the install projection manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionEntry {
    /// Kind of asset this entry describes.
    pub kind: InstallDriftAssetKind,
    /// Stable category identifying the projection type.
    pub category: AssetCategory,
    /// Destination path relative to the repository root.
    pub rel_path: &'static str,
    /// Canonical expected content for generated assets.
    pub expected_content: Option<&'static str>,
    /// Content ownership after installation.
    pub ownership: AssetOwnership,
    /// How drift is evaluated for this asset.
    pub drift_policy: EntryDriftPolicy,
}

/// A user-editable preserved standard that Tanren requires to be present
/// but does not own the content of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreservedInputEntry {
    /// Destination path relative to the repository root.
    pub rel_path: &'static str,
}

/// The canonical install projection manifest — Tanren-owned generated
/// projections only. Every entry is an exact-match generated asset.
///
/// Entries are stored in deterministic lexicographic order by `rel_path`.
pub static PROJECTION_MANIFEST: &[ProjectionEntry] = &[
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".claude/commands/architect-system.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/architect-system.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".claude/commands/craft-roadmap.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/craft-roadmap.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".claude/commands/identify-behaviors.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/identify-behaviors.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".claude/commands/plan-product.md",
        expected_content: Some(include_str!("../../../../commands/project/plan-product.md")),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".codex/skills/architect-system.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/architect-system.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".codex/skills/craft-roadmap.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/craft-roadmap.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".codex/skills/identify-behaviors.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/identify-behaviors.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".codex/skills/plan-product.md",
        expected_content: Some(include_str!("../../../../commands/project/plan-product.md")),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".opencode/commands/architect-system.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/architect-system.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".opencode/commands/craft-roadmap.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/craft-roadmap.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".opencode/commands/identify-behaviors.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/identify-behaviors.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        category: AssetCategory::CommandProjection,
        rel_path: ".opencode/commands/plan-product.md",
        expected_content: Some(include_str!("../../../../commands/project/plan-product.md")),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
];

/// Required user-editable preserved standards. These are presence-checked
/// during drift evaluation and reported separately from Tanren-owned
/// projections. User edits to these files are accepted as non-drift.
pub static PRESERVED_INPUTS: &[PreservedInputEntry] = &[PreservedInputEntry {
    rel_path: "docs/standards/global/tech-stack.md",
}];
