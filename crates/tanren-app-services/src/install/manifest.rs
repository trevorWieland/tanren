//! Install projection manifest — the single authoritative catalog of generated
//! and preserved assets that `tanren install` projects into a repository.
//!
//! Every interface binary and test harness resolves asset expectations through
//! [`PROJECTION_MANIFEST`] rather than maintaining separate hard-coded copies.
//! The manifest is a static slice so no heap allocation is required per access.

use tanren_contract::InstallDriftAssetKind;

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

/// Describes a single entry in the install projection manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionEntry {
    /// Kind of asset this entry describes.
    pub kind: InstallDriftAssetKind,
    /// Destination path relative to the repository root.
    pub rel_path: &'static str,
    /// Canonical expected content for generated assets.
    /// `None` for preserved standards (user owns the content).
    pub expected_content: Option<&'static str>,
    /// Content ownership after installation.
    pub ownership: AssetOwnership,
    /// How drift is evaluated for this asset.
    pub drift_policy: EntryDriftPolicy,
}

/// The canonical install projection manifest — one static source of truth
/// for all generated and preserved assets that `tanren install` projects
/// into a repository.
///
/// Generated command assets carry their expected content inlined from the
/// canonical command sources. Preserved standards carry no expected content;
/// drift is evaluated by presence only.
pub static PROJECTION_MANIFEST: &[ProjectionEntry] = &[
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        rel_path: ".claude/commands/architect-system.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/architect-system.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        rel_path: ".claude/commands/craft-roadmap.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/craft-roadmap.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        rel_path: ".claude/commands/identify-behaviors.md",
        expected_content: Some(include_str!(
            "../../../../commands/project/identify-behaviors.md"
        )),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::Generated,
        rel_path: ".claude/commands/plan-product.md",
        expected_content: Some(include_str!("../../../../commands/project/plan-product.md")),
        ownership: AssetOwnership::Tanren,
        drift_policy: EntryDriftPolicy::ExactMatch,
    },
    ProjectionEntry {
        kind: InstallDriftAssetKind::PreservedStandard,
        rel_path: "docs/standards/global/tech-stack.md",
        expected_content: None,
        ownership: AssetOwnership::User,
        drift_policy: EntryDriftPolicy::PresenceOnly,
    },
];
