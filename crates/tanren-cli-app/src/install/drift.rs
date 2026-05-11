//! Read-only install drift analysis with a drift-owned classification model.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::install::error::InstallDriftError;
use crate::install::manifest::{PreservationPolicy, RepoRelativePath};
use crate::install::plan::PlannedWriteKind;
use crate::install::plan_install;

/// Typed drift status for one install-managed repository asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallDriftStatus {
    /// Repository file matches the current install projection.
    Clean,
    /// Tanren-owned generated file exists but content differs from projection.
    ChangedGeneratedAsset,
    /// Tanren-owned generated file is missing from the repository.
    MissingGeneratedAsset,
    /// Preserved standards profile file is missing from the repository.
    MissingPreservedStandard,
    /// Preserved standards profile file was edited and accepted as user-owned.
    AcceptedPreservedEdit,
}

impl InstallDriftStatus {
    /// Whether this status should count as drift.
    #[must_use]
    pub const fn is_drift(self) -> bool {
        matches!(
            self,
            Self::ChangedGeneratedAsset
                | Self::MissingGeneratedAsset
                | Self::MissingPreservedStandard
        )
    }

    /// Render this status to its canonical CLI label.
    ///
    /// The label is the single source of truth used by the CLI writer
    /// and the BDD witness parser. Adding a new drift status requires
    /// exactly one table entry plus one enum variant.
    #[must_use]
    pub fn to_label(self) -> &'static str {
        status_label(self)
    }

    /// Parse a canonical CLI label back to the typed status.
    ///
    /// Returns `None` for unknown labels so callers can decide
    /// how to handle unrecognized output.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        status_from_label(label)
    }
}

/// Single mapping table between [`InstallDriftStatus`] variants and CLI labels.
///
/// Adding a new drift status requires editing exactly one row here
/// plus adding the enum variant — no scattered match arms elsewhere.
const STATUS_LABELS: &[(InstallDriftStatus, &str)] = &[
    (InstallDriftStatus::Clean, "clean"),
    (
        InstallDriftStatus::ChangedGeneratedAsset,
        "changed_generated",
    ),
    (
        InstallDriftStatus::MissingGeneratedAsset,
        "missing_generated",
    ),
    (
        InstallDriftStatus::MissingPreservedStandard,
        "missing_preserved",
    ),
    (
        InstallDriftStatus::AcceptedPreservedEdit,
        "accepted_preserved",
    ),
];

/// Look up the canonical CLI label for a drift status using the shared table.
fn status_label(status: InstallDriftStatus) -> &'static str {
    STATUS_LABELS
        .iter()
        .find_map(|&(variant, label)| (variant == status).then_some(label))
        .expect("STATUS_LABELS must cover every InstallDriftStatus variant")
}

/// Look up the typed status from a canonical CLI label using the shared table.
fn status_from_label(label: &str) -> Option<InstallDriftStatus> {
    STATUS_LABELS
        .iter()
        .find(|&&(_, l)| l == label)
        .map(|&(variant, _)| variant)
}

/// Observed on-disk state for one install-managed asset, used as
/// classification input by the drift analyzer.
///
/// This is a drift-owned model: it captures only what classification
/// needs — not install-planning write kind or apply-side metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskState {
    /// File exists on disk with content matching the projection hash.
    MatchesProjection,
    /// File exists on disk but content differs from the projection hash.
    DiffersFromProjection,
    /// File does not exist on disk.
    Absent,
}

/// Drift-owned input model carrying exactly what classification needs
/// for one install-managed asset.
#[derive(Debug, Clone)]
pub struct DriftAssetState {
    preservation: PreservationPolicy,
    disk_state: DiskState,
    was_preserved: bool,
}

impl DriftAssetState {
    /// Build a drift asset state from its classification inputs.
    #[must_use]
    pub fn new(
        preservation: PreservationPolicy,
        disk_state: DiskState,
        was_preserved: bool,
    ) -> Self {
        Self {
            preservation,
            disk_state,
            was_preserved,
        }
    }

    /// The preservation policy for this asset.
    #[must_use]
    pub const fn preservation(&self) -> PreservationPolicy {
        self.preservation
    }

    /// The observed on-disk state for this asset.
    #[must_use]
    pub const fn disk_state(&self) -> DiskState {
        self.disk_state
    }

    /// Whether the asset was previously preserved due to user edits.
    #[must_use]
    pub const fn was_preserved(&self) -> bool {
        self.was_preserved
    }
}

/// Drift status for one install-managed repository path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallDriftEntry {
    path: RepoRelativePath,
    status: InstallDriftStatus,
}

impl InstallDriftEntry {
    /// Repo-relative path for this drift status row.
    #[must_use]
    pub fn path(&self) -> &RepoRelativePath {
        &self.path
    }

    /// Typed drift status for this row.
    #[must_use]
    pub const fn status(&self) -> InstallDriftStatus {
        self.status
    }
}

/// Precomputed drift summary counts — O(1) accessors with no per-call iteration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct DriftCounts {
    clean: usize,
    changed_generated: usize,
    missing_generated: usize,
    missing_preserved: usize,
    accepted_preserved: usize,
    drift: usize,
}

/// Read-only drift report for install-managed repository assets.
///
/// Category counts are precomputed once at construction time so
/// `has_drift`, `drift_count`, and per-status count accessors are O(1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallDriftReport {
    entries: Vec<InstallDriftEntry>,
    counts: DriftCounts,
}

impl InstallDriftReport {
    /// One row per install-managed asset in deterministic path order.
    #[must_use]
    pub fn entries(&self) -> &[InstallDriftEntry] {
        &self.entries
    }

    /// Number of entries with `Clean` status.
    #[must_use]
    pub fn clean_count(&self) -> usize {
        self.counts.clean
    }

    /// Number of entries with `ChangedGeneratedAsset` status.
    #[must_use]
    pub fn changed_generated_count(&self) -> usize {
        self.counts.changed_generated
    }

    /// Number of entries with `MissingGeneratedAsset` status.
    #[must_use]
    pub fn missing_generated_count(&self) -> usize {
        self.counts.missing_generated
    }

    /// Number of entries with `MissingPreservedStandard` status.
    #[must_use]
    pub fn missing_preserved_count(&self) -> usize {
        self.counts.missing_preserved
    }

    /// Number of entries with `AcceptedPreservedEdit` status.
    #[must_use]
    pub fn accepted_preserved_count(&self) -> usize {
        self.counts.accepted_preserved
    }

    /// Number of entries currently classified as drift.
    #[must_use]
    pub fn drift_count(&self) -> usize {
        self.counts.drift
    }

    /// Whether drift exists for any install-managed path.
    #[must_use]
    pub fn has_drift(&self) -> bool {
        self.counts.drift > 0
    }
}

/// Validate install inputs and analyze drift without mutating repository files.
pub(super) fn check_install_drift(
    repository: &Path,
    profile: &str,
    integration_selection: Option<&str>,
) -> Result<InstallDriftReport, InstallDriftError> {
    let plan = plan_install(repository, profile, integration_selection)?;

    let asset_states = build_drift_asset_states(&plan);

    let mut entries = Vec::with_capacity(asset_states.len());
    let mut counts = DriftCounts::default();

    for (path, asset_state) in &asset_states {
        let status = classify_status(asset_state);
        if status.is_drift() {
            counts.drift += 1;
        }
        match status {
            InstallDriftStatus::Clean => counts.clean += 1,
            InstallDriftStatus::ChangedGeneratedAsset => counts.changed_generated += 1,
            InstallDriftStatus::MissingGeneratedAsset => counts.missing_generated += 1,
            InstallDriftStatus::MissingPreservedStandard => counts.missing_preserved += 1,
            InstallDriftStatus::AcceptedPreservedEdit => counts.accepted_preserved += 1,
        }
        entries.push(InstallDriftEntry {
            path: path.clone(),
            status,
        });
    }

    entries.sort_by(|left, right| left.path().as_str().cmp(right.path().as_str()));
    Ok(InstallDriftReport { entries, counts })
}

/// Build drift-owned asset states from the validated install plan.
///
/// Translates install-plan write kind and preserved-set into the
/// drift-specific [`DiskState`] model so classification reads from
/// a drift-owned type rather than from [`PlannedWriteKind`] directly.
fn build_drift_asset_states(
    plan: &crate::install::InstallPlan,
) -> Vec<(RepoRelativePath, DriftAssetState)> {
    let writes: BTreeMap<&RepoRelativePath, PlannedWriteKind> = plan
        .writes()
        .iter()
        .map(|write| (write.path(), write.kind()))
        .collect();
    let preserved: BTreeSet<&RepoRelativePath> = plan.preserved().iter().collect();

    plan.manifest()
        .entries
        .iter()
        .map(|manifest_entry| {
            let write_kind = writes.get(&manifest_entry.path).copied();
            let was_preserved = preserved.contains(&manifest_entry.path);
            let disk_state = translate_disk_state(write_kind);
            let asset_state =
                DriftAssetState::new(manifest_entry.preservation, disk_state, was_preserved);
            (manifest_entry.path.clone(), asset_state)
        })
        .collect()
}

/// Translate install-plan write kind into drift-owned disk state.
fn translate_disk_state(write_kind: Option<PlannedWriteKind>) -> DiskState {
    match write_kind {
        None => DiskState::MatchesProjection,
        Some(PlannedWriteKind::Updated) => DiskState::DiffersFromProjection,
        Some(PlannedWriteKind::Created | PlannedWriteKind::Restored) => DiskState::Absent,
    }
}

/// Classify drift status from the drift-owned asset state model.
fn classify_status(asset_state: &DriftAssetState) -> InstallDriftStatus {
    match asset_state.preservation() {
        PreservationPolicy::ReplaceGenerated => match asset_state.disk_state() {
            DiskState::DiffersFromProjection => InstallDriftStatus::ChangedGeneratedAsset,
            DiskState::Absent => InstallDriftStatus::MissingGeneratedAsset,
            DiskState::MatchesProjection => InstallDriftStatus::Clean,
        },
        PreservationPolicy::PreserveUserEdits => match asset_state.disk_state() {
            DiskState::Absent => InstallDriftStatus::MissingPreservedStandard,
            DiskState::DiffersFromProjection => InstallDriftStatus::AcceptedPreservedEdit,
            DiskState::MatchesProjection if asset_state.was_preserved() => {
                InstallDriftStatus::AcceptedPreservedEdit
            }
            DiskState::MatchesProjection => InstallDriftStatus::Clean,
        },
    }
}
