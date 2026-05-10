use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::install::catalog::{
    build_install_asset_catalog, build_trusted_generated_asset_registry,
};
use crate::install::error::InstallError;
use crate::install::manifest::{
    CURRENT_MANIFEST_SCHEMA_COMPATIBILITY, CatalogSchemaRevision, INSTALL_MANIFEST_REPO_PATH,
    INSTALL_MANIFEST_VERSION, InstallAssetProjection, InstallManifest, ManifestEntry,
    PreservationPolicy, RepoRelativePath, Sha256Hex, build_manifest_entries, sha256_hex,
};
use crate::install::path_guard::resolve_repo_path;

/// Upgrade planning result for repositories with and without install state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradePlanOutcome {
    NothingToUpgrade(NothingToUpgrade),
    Planned(Box<UpgradePlan>),
}

/// Clear no-op outcome for repositories that were never installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NothingToUpgrade {
    manifest_path: RepoRelativePath,
}

impl NothingToUpgrade {
    /// Expected install manifest path.
    #[must_use]
    pub fn manifest_path(&self) -> &RepoRelativePath {
        &self.manifest_path
    }
}

/// Planned upgrade write action category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradePlannedWriteKind {
    Created,
    Updated,
    Restored,
}

/// One planned write operation for upgrade apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradePlannedWrite {
    path: RepoRelativePath,
    absolute_path: PathBuf,
    content: &'static str,
    kind: UpgradePlannedWriteKind,
}

impl UpgradePlannedWrite {
    #[must_use]
    pub fn path(&self) -> &RepoRelativePath {
        &self.path
    }

    #[must_use]
    pub const fn kind(&self) -> UpgradePlannedWriteKind {
        self.kind
    }

    #[must_use]
    pub fn absolute_path(&self) -> &Path {
        &self.absolute_path
    }

    #[must_use]
    pub fn content_bytes(&self) -> &'static [u8] {
        self.content.as_bytes()
    }
}

/// One planned stale generated-file removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradePlannedRemoval {
    path: RepoRelativePath,
    absolute_path: PathBuf,
}

impl UpgradePlannedRemoval {
    #[must_use]
    pub fn path(&self) -> &RepoRelativePath {
        &self.path
    }

    #[must_use]
    pub fn absolute_path(&self) -> &Path {
        &self.absolute_path
    }
}

/// Compatibility or manual-action concern reported during preview planning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationConcern {
    CatalogRevisionChanged {
        from: CatalogSchemaRevision,
        to: CatalogSchemaRevision,
    },
    PreservedUserEdit {
        path: RepoRelativePath,
    },
    DriftedStaleGeneratedAsset {
        path: RepoRelativePath,
    },
}

/// Upgrade plan produced after manifest-aware reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradePlan {
    repository_root: PathBuf,
    manifest_path: RepoRelativePath,
    manifest_absolute_path: PathBuf,
    previous_manifest: InstallManifest,
    next_manifest: InstallManifest,
    writes: Vec<UpgradePlannedWrite>,
    removals: Vec<UpgradePlannedRemoval>,
    preserved: Vec<RepoRelativePath>,
    migration_concerns: Vec<MigrationConcern>,
}

impl UpgradePlan {
    #[must_use]
    pub fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    #[must_use]
    pub fn manifest_path(&self) -> &RepoRelativePath {
        &self.manifest_path
    }

    #[must_use]
    pub fn manifest_absolute_path(&self) -> &Path {
        &self.manifest_absolute_path
    }

    #[must_use]
    pub fn previous_manifest(&self) -> &InstallManifest {
        &self.previous_manifest
    }

    #[must_use]
    pub fn next_manifest(&self) -> &InstallManifest {
        &self.next_manifest
    }

    #[must_use]
    pub fn writes(&self) -> &[UpgradePlannedWrite] {
        &self.writes
    }

    #[must_use]
    pub fn removals(&self) -> &[UpgradePlannedRemoval] {
        &self.removals
    }

    #[must_use]
    pub fn preserved(&self) -> &[RepoRelativePath] {
        &self.preserved
    }

    #[must_use]
    pub fn migration_concerns(&self) -> &[MigrationConcern] {
        &self.migration_concerns
    }
}

/// Build a manifest-aware upgrade plan without mutating repository files.
pub fn plan_upgrade(repository: &Path) -> Result<UpgradePlanOutcome, InstallError> {
    let repository_root = validate_repository_root(repository)?;
    let manifest_path = RepoRelativePath::parse(INSTALL_MANIFEST_REPO_PATH)?;
    let manifest_absolute_path = resolve_repo_path(&repository_root, &manifest_path)?;

    let Some(previous_manifest) = load_previous_manifest(&manifest_absolute_path)? else {
        return Ok(UpgradePlanOutcome::NothingToUpgrade(NothingToUpgrade {
            manifest_path,
        }));
    };

    validate_manifest_version(&previous_manifest)?;

    let integrations = previous_manifest
        .integrations
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let target_assets = build_install_asset_catalog(previous_manifest.profile, &integrations)?;
    let trusted_generated_asset_registry = build_trusted_generated_asset_registry()?;

    let planned = plan_upgrade_with_catalog(
        &repository_root,
        manifest_path,
        manifest_absolute_path,
        previous_manifest,
        target_assets,
        &trusted_generated_asset_registry,
    )?;
    Ok(UpgradePlanOutcome::Planned(Box::new(planned)))
}

fn plan_upgrade_with_catalog(
    repository_root: &Path,
    manifest_path: RepoRelativePath,
    manifest_absolute_path: PathBuf,
    previous_manifest: InstallManifest,
    mut target_assets: Vec<InstallAssetProjection>,
    trusted_generated_asset_registry: &BTreeSet<RepoRelativePath>,
) -> Result<UpgradePlan, InstallError> {
    target_assets.sort_by(|left, right| {
        left.destination_path
            .as_str()
            .cmp(right.destination_path.as_str())
    });

    let mut next_entries = build_manifest_entries(&target_assets);
    next_entries.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));

    let previous_entries_by_path = build_previous_entry_map(&previous_manifest)?;
    let desired_generated_paths = next_entries
        .iter()
        .filter(|entry| entry.preservation == PreservationPolicy::ReplaceGenerated)
        .map(|entry| entry.path.as_str())
        .collect::<BTreeSet<_>>();

    let mut writes = Vec::new();
    let mut preserved = Vec::new();
    let mut migration_concerns = Vec::new();

    for (asset, next_entry) in target_assets.iter().zip(&next_entries) {
        let absolute = resolve_repo_path(repository_root, &asset.destination_path)?;
        let previous_entry = previous_entries_by_path
            .get(asset.destination_path.as_str())
            .copied();
        let decision = decide_upgrade_write(absolute, asset, next_entry, previous_entry)?;
        collect_decision(
            decision,
            &mut writes,
            &mut preserved,
            &mut migration_concerns,
        );
    }

    let stale = plan_stale_generated_removals(
        repository_root,
        &previous_manifest,
        &desired_generated_paths,
        trusted_generated_asset_registry,
    )?;
    preserved.extend(stale.preserved_paths);
    migration_concerns.extend(stale.concerns);

    if previous_manifest.compatibility.catalog_revision
        != CURRENT_MANIFEST_SCHEMA_COMPATIBILITY.catalog_revision
    {
        migration_concerns.push(MigrationConcern::CatalogRevisionChanged {
            from: previous_manifest.compatibility.catalog_revision,
            to: CURRENT_MANIFEST_SCHEMA_COMPATIBILITY.catalog_revision,
        });
    }

    preserved.sort();
    preserved.dedup();
    writes.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));

    let next_manifest = InstallManifest::from_entries(
        previous_manifest.profile,
        previous_manifest.integrations.clone(),
        next_entries,
    );

    Ok(UpgradePlan {
        repository_root: repository_root.to_path_buf(),
        manifest_path,
        manifest_absolute_path,
        previous_manifest,
        next_manifest,
        writes,
        removals: stale.removals,
        preserved,
        migration_concerns,
    })
}

fn validate_repository_root(repository: &Path) -> Result<PathBuf, InstallError> {
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

fn load_previous_manifest(
    manifest_absolute_path: &Path,
) -> Result<Option<InstallManifest>, InstallError> {
    if !manifest_absolute_path.exists() {
        return Ok(None);
    }

    let raw_manifest =
        fs::read_to_string(manifest_absolute_path).map_err(|err| InstallError::ReadFailure {
            path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
            message: err.to_string(),
        })?;

    toml::from_str(&raw_manifest)
        .map(Some)
        .map_err(|err| InstallError::InvalidInstallManifest {
            path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
            message: err.to_string(),
        })
}

fn validate_manifest_version(manifest: &InstallManifest) -> Result<(), InstallError> {
    if manifest.manifest_version != INSTALL_MANIFEST_VERSION {
        return Err(InstallError::InvalidInstallManifest {
            path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
            message: format!("unsupported manifest version {}", manifest.manifest_version),
        });
    }
    Ok(())
}

fn build_previous_entry_map(
    manifest: &InstallManifest,
) -> Result<BTreeMap<&str, &ManifestEntry>, InstallError> {
    let mut entries = BTreeMap::new();
    for entry in &manifest.entries {
        let path = entry.path.as_str();
        if entries.insert(path, entry).is_some() {
            return Err(InstallError::InvalidInstallManifest {
                path: INSTALL_MANIFEST_REPO_PATH.to_owned(),
                message: format!("duplicate manifest entry for '{path}'"),
            });
        }
    }
    Ok(entries)
}

enum UpgradeWriteDecision {
    Write(UpgradePlannedWrite),
    Preserve(RepoRelativePath),
    Unchanged,
}

fn decide_upgrade_write(
    absolute_path: PathBuf,
    asset: &InstallAssetProjection,
    next_entry: &ManifestEntry,
    previous_entry: Option<&ManifestEntry>,
) -> Result<UpgradeWriteDecision, InstallError> {
    if !absolute_path.exists() {
        let kind = if previous_entry.is_some() {
            UpgradePlannedWriteKind::Restored
        } else {
            UpgradePlannedWriteKind::Created
        };
        return Ok(UpgradeWriteDecision::Write(UpgradePlannedWrite {
            path: asset.destination_path.clone(),
            absolute_path,
            content: asset.content,
            kind,
        }));
    }

    let current_hash = hash_current_file(&absolute_path, asset.destination_path.as_str())?;
    if current_hash == next_entry.content_hash {
        return Ok(UpgradeWriteDecision::Unchanged);
    }

    if asset.preservation == PreservationPolicy::PreserveUserEdits {
        let first_manifest_tracking = previous_entry.is_none();
        let tracked_entry_drifted =
            previous_entry.is_some_and(|entry| current_hash != entry.content_hash);
        if first_manifest_tracking || tracked_entry_drifted {
            return Ok(UpgradeWriteDecision::Preserve(
                asset.destination_path.clone(),
            ));
        }
    }

    Ok(UpgradeWriteDecision::Write(UpgradePlannedWrite {
        path: asset.destination_path.clone(),
        absolute_path,
        content: asset.content,
        kind: UpgradePlannedWriteKind::Updated,
    }))
}

fn collect_decision(
    decision: UpgradeWriteDecision,
    writes: &mut Vec<UpgradePlannedWrite>,
    preserved: &mut Vec<RepoRelativePath>,
    concerns: &mut Vec<MigrationConcern>,
) {
    match decision {
        UpgradeWriteDecision::Write(write) => writes.push(write),
        UpgradeWriteDecision::Preserve(path) => {
            concerns.push(MigrationConcern::PreservedUserEdit { path: path.clone() });
            preserved.push(path);
        }
        UpgradeWriteDecision::Unchanged => {}
    }
}

struct StaleRemovalPlan {
    removals: Vec<UpgradePlannedRemoval>,
    preserved_paths: Vec<RepoRelativePath>,
    concerns: Vec<MigrationConcern>,
}

fn plan_stale_generated_removals(
    repository_root: &Path,
    previous_manifest: &InstallManifest,
    desired_generated_paths: &BTreeSet<&str>,
    trusted_generated_asset_registry: &BTreeSet<RepoRelativePath>,
) -> Result<StaleRemovalPlan, InstallError> {
    let mut removals = Vec::new();
    let mut preserved_paths = Vec::new();
    let mut concerns = Vec::new();

    for entry in &previous_manifest.entries {
        let path = entry.path.as_str();
        if desired_generated_paths.contains(path)
            || entry.preservation != PreservationPolicy::ReplaceGenerated
        {
            continue;
        }

        if !trusted_generated_asset_registry.contains(&entry.path) {
            continue;
        }

        let absolute = resolve_repo_path(repository_root, &entry.path)?;
        if !absolute.exists() {
            continue;
        }

        let current_hash = hash_current_file(&absolute, entry.path.as_str())?;
        if current_hash != entry.content_hash {
            concerns.push(MigrationConcern::DriftedStaleGeneratedAsset {
                path: entry.path.clone(),
            });
            preserved_paths.push(entry.path.clone());
            continue;
        }

        removals.push(UpgradePlannedRemoval {
            path: entry.path.clone(),
            absolute_path: absolute,
        });
    }

    removals.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));
    preserved_paths.sort();
    preserved_paths.dedup();
    Ok(StaleRemovalPlan {
        removals,
        preserved_paths,
        concerns,
    })
}

fn hash_current_file(path: &Path, display_path: &str) -> Result<Sha256Hex, InstallError> {
    let current = fs::read(path).map_err(|err| InstallError::ReadFailure {
        path: display_path.to_owned(),
        message: err.to_string(),
    })?;
    Ok(sha256_hex(&current))
}

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}
