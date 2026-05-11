//! Install planning with validation and manifest-aware reconciliation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::install::catalog::{build_install_asset_catalog, trusted_generated_asset_registry};
use crate::install::error::InstallError;
use crate::install::manifest::{
    INSTALL_MANIFEST_REPO_PATH, InstallAssetProjection, InstallManifest, ManifestEntry,
    PreservationPolicy, RepoRelativePath, Sha256Hex, build_manifest_entries, sha256_hex_file,
};
use crate::install::manifest_entry_contract::{
    ManifestEntryKind, ValidatedManifestEntry, validate_manifest_entry_contract,
};
use crate::install::manifest_migration::{
    ManifestLoadOutcome, ManifestMigrationOutcome, ValidatedInstallManifest, migrate_manifest,
    validate_migrated_manifest,
};
use crate::install::path_guard::resolve_repo_path;
use crate::install::{InstallIntegration, InstallProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedWriteKind {
    Created,
    Updated,
    Restored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWrite {
    path: RepoRelativePath,
    absolute_path: PathBuf,
    content: &'static str,
    kind: PlannedWriteKind,
}

impl PlannedWrite {
    #[must_use]
    pub fn path(&self) -> &RepoRelativePath {
        &self.path
    }

    #[must_use]
    pub const fn kind(&self) -> PlannedWriteKind {
        self.kind
    }

    /// Absolute output path validated during planning.
    #[must_use]
    pub(crate) fn absolute_path(&self) -> &Path {
        &self.absolute_path
    }

    /// Planned UTF-8 content bytes.
    #[must_use]
    pub(crate) fn content_bytes(&self) -> &'static [u8] {
        self.content.as_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRemoval {
    path: RepoRelativePath,
    absolute_path: PathBuf,
}

impl PlannedRemoval {
    #[must_use]
    pub fn path(&self) -> &RepoRelativePath {
        &self.path
    }

    /// Absolute path validated during planning.
    #[must_use]
    pub(crate) fn absolute_path(&self) -> &Path {
        &self.absolute_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    repository_root: PathBuf,
    writes: Vec<PlannedWrite>,
    removals: Vec<PlannedRemoval>,
    preserved: Vec<RepoRelativePath>,
    manifest_path: RepoRelativePath,
    manifest_absolute_path: PathBuf,
    manifest: InstallManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepositoryInstallState {
    repository_root: PathBuf,
    manifest_path: RepoRelativePath,
    manifest_absolute_path: PathBuf,
    previous_manifest: Option<ValidatedInstallManifest>,
}
impl RepositoryInstallState {
    #[must_use]
    pub(super) fn previous_manifest(&self) -> Option<&InstallManifest> {
        self.previous_manifest
            .as_ref()
            .map(ValidatedInstallManifest::inner)
    }
}
impl InstallPlan {
    #[must_use]
    pub(crate) fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    #[must_use]
    pub fn writes(&self) -> &[PlannedWrite] {
        &self.writes
    }

    #[must_use]
    pub fn removals(&self) -> &[PlannedRemoval] {
        &self.removals
    }

    #[must_use]
    pub fn preserved(&self) -> &[RepoRelativePath] {
        &self.preserved
    }

    #[must_use]
    pub fn manifest_path(&self) -> &RepoRelativePath {
        &self.manifest_path
    }

    #[must_use]
    pub(crate) fn manifest_absolute_path(&self) -> &Path {
        &self.manifest_absolute_path
    }

    #[must_use]
    pub fn manifest(&self) -> &InstallManifest {
        &self.manifest
    }
}

pub(super) fn build_install_plan(
    repository: &Path,
    profile: InstallProfile,
    integrations: &BTreeSet<InstallIntegration>,
) -> Result<InstallPlan, InstallError> {
    let state = load_repository_install_state(repository)?;
    build_install_plan_from_state(state, profile, integrations)
}
pub(super) fn load_repository_install_state(
    repository: &Path,
) -> Result<RepositoryInstallState, InstallError> {
    let repository_root = validate_repository_root(repository)?;
    let manifest_path = RepoRelativePath::parse(INSTALL_MANIFEST_REPO_PATH)?;
    let manifest_absolute_path = resolve_repo_path(&repository_root, &manifest_path)?;
    let load_outcome =
        load_and_validate_previous_manifest(&manifest_path, &manifest_absolute_path)?;
    let previous_manifest = load_outcome.require_validated(manifest_path.as_str())?;

    Ok(RepositoryInstallState {
        repository_root,
        manifest_path,
        manifest_absolute_path,
        previous_manifest,
    })
}
pub(super) fn build_install_plan_from_state(
    state: RepositoryInstallState,
    profile: InstallProfile,
    integrations: &BTreeSet<InstallIntegration>,
) -> Result<InstallPlan, InstallError> {
    let RepositoryInstallState {
        repository_root,
        manifest_path,
        manifest_absolute_path,
        previous_manifest,
    } = state;

    let mut assets = build_install_asset_catalog(profile, integrations)?;
    assets.sort_by(|left, right| {
        left.destination_path
            .as_str()
            .cmp(right.destination_path.as_str())
    });

    let mut manifest_entries = build_manifest_entries(&assets);
    manifest_entries.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));

    let previous_entries_by_path = build_previous_entry_map(
        &manifest_path,
        previous_manifest
            .as_ref()
            .map(ValidatedInstallManifest::inner),
    )?;
    let desired_generated_paths = manifest_entries
        .iter()
        .filter(|entry| entry.preservation == PreservationPolicy::ReplaceGenerated)
        .map(|entry| entry.path.as_str())
        .collect::<BTreeSet<_>>();
    let trusted_generated_asset_registry = trusted_generated_asset_registry();

    let writes = build_write_plan(
        &repository_root,
        &assets,
        &manifest_entries,
        &previous_entries_by_path,
    )?;
    let stale_removal_plan = build_removals(
        &repository_root,
        &desired_generated_paths,
        trusted_generated_asset_registry,
        &previous_entries_by_path,
        &manifest_path,
    )?;
    let (writes, mut preserved) = split_planned_asset_actions(writes);
    preserved.extend(stale_removal_plan.preserved_paths);
    preserved.sort();
    preserved.dedup();

    let manifest = InstallManifest::from_entries(
        profile,
        integrations.iter().copied().collect(),
        manifest_entries,
    );

    Ok(InstallPlan {
        repository_root,
        writes,
        removals: stale_removal_plan.removals,
        preserved,
        manifest_path,
        manifest_absolute_path,
        manifest,
    })
}

#[derive(Debug, Clone)]
enum PlannedAssetAction {
    Write(PlannedWrite),
    Preserve(RepoRelativePath),
    Unchanged,
}
pub(super) fn validate_repository_root(repository: &Path) -> Result<PathBuf, InstallError> {
    let canonical =
        repository
            .canonicalize()
            .map_err(|err| InstallError::InvalidRepositoryPath {
                path: format!("{} ({err})", display_repository_argument(repository)),
            })?;

    if !canonical.is_dir() {
        return Err(InstallError::RepositoryPathNotDirectory {
            path: display_repository_argument(repository),
        });
    }

    Ok(canonical)
}
fn load_and_validate_previous_manifest(
    manifest_path: &RepoRelativePath,
    manifest_absolute_path: &Path,
) -> Result<ManifestLoadOutcome, InstallError> {
    if !manifest_absolute_path.exists() {
        return Ok(ManifestLoadOutcome::NoManifest);
    }

    let raw_manifest =
        fs::read_to_string(manifest_absolute_path).map_err(|err| InstallError::ReadFailure {
            path: manifest_path.as_str().to_owned(),
            message: err.to_string(),
        })?;

    let manifest: InstallManifest =
        toml::from_str(&raw_manifest).map_err(|err| InstallError::InvalidInstallManifest {
            path: manifest_path.as_str().to_owned(),
            message: err.to_string(),
        })?;
    let migration_outcome = migrate_manifest(manifest);
    match migration_outcome {
        ManifestMigrationOutcome::CurrentVersion { manifest }
        | ManifestMigrationOutcome::Migrated { manifest, .. } => {
            let validated = validate_migrated_manifest(manifest, manifest_path.as_str())?;
            Ok(ManifestLoadOutcome::Validated(validated))
        }
        ManifestMigrationOutcome::UnsupportedVersion {
            detected,
            min_supported,
            current,
        } => Ok(ManifestLoadOutcome::UnsupportedVersion {
            detected,
            min_supported,
            current,
        }),
    }
}
fn build_previous_entry_map<'a>(
    manifest_path: &RepoRelativePath,
    manifest: Option<&'a InstallManifest>,
) -> Result<BTreeMap<&'a str, ValidatedManifestEntry<'a>>, InstallError> {
    let Some(previous_manifest) = manifest else {
        return Ok(BTreeMap::new());
    };

    let mut entries = BTreeMap::new();
    for entry in &previous_manifest.entries {
        let kind = validate_manifest_entry_contract(
            entry,
            previous_manifest.profile,
            manifest_path.as_str(),
        )?;
        let path = entry.path.as_str();
        if entries
            .insert(path, ValidatedManifestEntry { entry, kind })
            .is_some()
        {
            return Err(InstallError::InvalidInstallManifest {
                path: manifest_path.as_str().to_owned(),
                message: format!("duplicate manifest entry for '{path}'"),
            });
        }
    }

    Ok(entries)
}
fn build_write_plan(
    repository_root: &Path,
    assets: &[InstallAssetProjection],
    manifest_entries: &[ManifestEntry],
    previous_entries_by_path: &BTreeMap<&str, ValidatedManifestEntry<'_>>,
) -> Result<Vec<PlannedAssetAction>, InstallError> {
    let mut actions = Vec::with_capacity(assets.len());

    for (asset, manifest_entry) in assets.iter().zip(manifest_entries) {
        let absolute_path = resolve_repo_path(repository_root, &asset.destination_path)?;
        let previous_entry = previous_entries_by_path
            .get(asset.destination_path.as_str())
            .map(|validated| validated.entry);
        let planned = plan_asset_write(absolute_path, asset, manifest_entry, previous_entry)?;
        actions.push(planned);
    }

    Ok(actions)
}
fn plan_asset_write(
    absolute_path: PathBuf,
    asset: &InstallAssetProjection,
    manifest_entry: &ManifestEntry,
    previous_entry: Option<&ManifestEntry>,
) -> Result<PlannedAssetAction, InstallError> {
    if !absolute_path.exists() {
        let kind = if previous_entry.is_some() {
            PlannedWriteKind::Restored
        } else {
            PlannedWriteKind::Created
        };
        return Ok(PlannedAssetAction::Write(PlannedWrite {
            path: asset.destination_path.clone(),
            absolute_path,
            content: asset.content,
            kind,
        }));
    }

    let current_hash = hash_current_file(&absolute_path, asset.destination_path.as_str())?;

    if current_hash == manifest_entry.content_hash {
        return Ok(PlannedAssetAction::Unchanged);
    }

    if asset.preservation == PreservationPolicy::PreserveUserEdits {
        let first_install_preserve = previous_entry.is_none();
        let previously_tracked_user_edit =
            previous_entry.is_some_and(|entry| current_hash != entry.content_hash);
        if first_install_preserve || previously_tracked_user_edit {
            return Ok(PlannedAssetAction::Preserve(asset.destination_path.clone()));
        }
    }

    Ok(PlannedAssetAction::Write(PlannedWrite {
        path: asset.destination_path.clone(),
        absolute_path,
        content: asset.content,
        kind: PlannedWriteKind::Updated,
    }))
}
fn split_planned_asset_actions(
    actions: Vec<PlannedAssetAction>,
) -> (Vec<PlannedWrite>, Vec<RepoRelativePath>) {
    let mut writes = Vec::new();
    let mut preserved = Vec::new();
    for action in actions {
        match action {
            PlannedAssetAction::Write(write) => writes.push(write),
            PlannedAssetAction::Preserve(path) => preserved.push(path),
            PlannedAssetAction::Unchanged => {}
        }
    }
    (writes, preserved)
}

#[derive(Debug, Clone)]
struct StaleRemovalPlan {
    removals: Vec<PlannedRemoval>,
    preserved_paths: Vec<RepoRelativePath>,
}
fn build_removals(
    repository_root: &Path,
    desired_generated_paths: &BTreeSet<&str>,
    trusted_generated_asset_registry: &BTreeSet<RepoRelativePath>,
    previous_entries_by_path: &BTreeMap<&str, ValidatedManifestEntry<'_>>,
    manifest_path: &RepoRelativePath,
) -> Result<StaleRemovalPlan, InstallError> {
    if previous_entries_by_path.is_empty() {
        return Ok(StaleRemovalPlan {
            removals: Vec::new(),
            preserved_paths: Vec::new(),
        });
    }

    let mut removals = Vec::with_capacity(previous_entries_by_path.len());
    let mut preserved_paths = Vec::new();
    for validated in previous_entries_by_path.values() {
        let entry = validated.entry;
        let path = entry.path.as_str();
        if desired_generated_paths.contains(path) {
            continue;
        }

        let ManifestEntryKind::GeneratedMethodologyCommand { integration } = validated.kind else {
            continue;
        };
        let _ = integration;

        if !trusted_generated_asset_registry.contains(&entry.path) {
            continue;
        }

        let absolute = resolve_repo_path(repository_root, &entry.path)?;
        if absolute.exists() {
            let current_hash = hash_current_file(&absolute, entry.path.as_str())?;
            if current_hash != entry.content_hash {
                preserved_paths.push(entry.path.clone());
                continue;
            }
            removals.push(PlannedRemoval {
                path: entry.path.clone(),
                absolute_path: absolute,
            });
        }
    }

    removals.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));
    preserved_paths.sort();
    preserved_paths.dedup();
    ensure_removals_unique(&removals, manifest_path)?;
    Ok(StaleRemovalPlan {
        removals,
        preserved_paths,
    })
}
fn hash_current_file(path: &Path, display_path: &str) -> Result<Sha256Hex, InstallError> {
    sha256_hex_file(path).map_err(|err| InstallError::ReadFailure {
        path: display_path.to_owned(),
        message: err.to_string(),
    })
}
fn ensure_removals_unique(
    removals: &[PlannedRemoval],
    manifest_path: &RepoRelativePath,
) -> Result<(), InstallError> {
    let mut seen = BTreeSet::new();
    for removal in removals {
        if !seen.insert(removal.path.as_str()) {
            return Err(InstallError::InvalidInstallManifest {
                path: manifest_path.as_str().to_owned(),
                message: format!("duplicate stale removal path '{}'", removal.path.as_str()),
            });
        }
    }
    Ok(())
}
fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}
