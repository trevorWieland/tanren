//! Install planning with validation and manifest-aware reconciliation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::install::catalog::build_install_asset_catalog;
use crate::install::error::InstallError;
use crate::install::manifest::{
    INSTALL_MANIFEST_REPO_PATH, INSTALL_MANIFEST_VERSION, InstallAssetProjection, InstallManifest,
    ManifestEntry, PreservationPolicy, RepoRelativePath, build_manifest_entries, sha256_hex,
};
use crate::install::{InstallIntegration, InstallProfile};

/// Planned file-write action category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedWriteKind {
    Created,
    Updated,
    Restored,
}

/// One planned write for a generated install asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWrite {
    pub path: RepoRelativePath,
    pub content: &'static str,
    pub kind: PlannedWriteKind,
}

/// Install plan produced after validating inputs and current repository state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    pub repository_root: PathBuf,
    pub writes: Vec<PlannedWrite>,
    pub removals: Vec<RepoRelativePath>,
    pub preserved: Vec<RepoRelativePath>,
    pub manifest_path: RepoRelativePath,
    pub manifest: InstallManifest,
}

/// Validate install inputs and repository state, then build the apply plan.
pub fn build_install_plan(
    repository: &Path,
    profile: InstallProfile,
    integrations: &BTreeSet<InstallIntegration>,
) -> Result<InstallPlan, InstallError> {
    let repository_root = validate_repository_root(repository)?;
    let mut assets = build_install_asset_catalog(profile, integrations)?;
    assets.sort_by(|left, right| {
        left.destination_path
            .as_str()
            .cmp(right.destination_path.as_str())
    });

    let mut manifest_entries = build_manifest_entries(&assets);
    manifest_entries.sort_by(|left, right| left.path.as_str().cmp(right.path.as_str()));

    let manifest_path = RepoRelativePath::parse(INSTALL_MANIFEST_REPO_PATH)?;
    let previous_manifest = load_previous_manifest(&repository_root, &manifest_path)?;
    validate_manifest_version(&repository_root, &manifest_path, previous_manifest.as_ref())?;

    let previous_entries_by_path =
        build_previous_entry_map(&repository_root, &manifest_path, previous_manifest.as_ref())?;
    let desired_paths = manifest_entries
        .iter()
        .map(|entry| entry.path.as_str().to_owned())
        .collect::<BTreeSet<_>>();

    let writes = build_write_plan(
        &repository_root,
        &assets,
        &manifest_entries,
        &previous_entries_by_path,
    )?;
    let removals = build_removals(
        &repository_root,
        &desired_paths,
        previous_manifest.as_ref(),
        &manifest_path,
    )?;
    let mut preserved = collect_preserved_paths(&writes);
    preserved.sort();

    let manifest = InstallManifest::from_entries(
        profile,
        integrations.iter().copied().collect(),
        manifest_entries,
    );

    Ok(InstallPlan {
        repository_root,
        writes: writes
            .into_iter()
            .filter_map(PlannedAssetAction::into_write)
            .collect(),
        removals,
        preserved,
        manifest_path,
        manifest,
    })
}

#[derive(Debug, Clone)]
enum PlannedAssetAction {
    Write(PlannedWrite),
    Preserve(RepoRelativePath),
    Unchanged,
}

impl PlannedAssetAction {
    fn into_write(self) -> Option<PlannedWrite> {
        match self {
            Self::Write(write) => Some(write),
            Self::Preserve(_) | Self::Unchanged => None,
        }
    }
}

fn validate_repository_root(repository: &Path) -> Result<PathBuf, InstallError> {
    let canonical =
        repository
            .canonicalize()
            .map_err(|err| InstallError::InvalidRepositoryPath {
                path: format!("{} ({err})", repository.display()),
            })?;

    if !canonical.is_dir() {
        return Err(InstallError::RepositoryPathNotDirectory {
            path: canonical.display().to_string(),
        });
    }

    Ok(canonical)
}

fn load_previous_manifest(
    repository_root: &Path,
    manifest_path: &RepoRelativePath,
) -> Result<Option<InstallManifest>, InstallError> {
    let absolute = repository_root.join(manifest_path.as_path());
    if !absolute.exists() {
        return Ok(None);
    }

    let raw_manifest = fs::read_to_string(&absolute).map_err(|err| InstallError::ReadFailure {
        path: absolute.display().to_string(),
        message: err.to_string(),
    })?;

    toml::from_str(&raw_manifest)
        .map(Some)
        .map_err(|err| InstallError::InvalidInstallManifest {
            path: absolute.display().to_string(),
            message: err.to_string(),
        })
}

fn validate_manifest_version(
    repository_root: &Path,
    manifest_path: &RepoRelativePath,
    manifest: Option<&InstallManifest>,
) -> Result<(), InstallError> {
    if let Some(previous_manifest) = manifest
        && previous_manifest.manifest_version != INSTALL_MANIFEST_VERSION
    {
        return Err(InstallError::InvalidInstallManifest {
            path: repository_root
                .join(manifest_path.as_path())
                .display()
                .to_string(),
            message: format!(
                "unsupported manifest version {}",
                previous_manifest.manifest_version
            ),
        });
    }
    Ok(())
}

fn build_previous_entry_map<'a>(
    repository_root: &Path,
    manifest_path: &RepoRelativePath,
    manifest: Option<&'a InstallManifest>,
) -> Result<BTreeMap<&'a str, &'a ManifestEntry>, InstallError> {
    let Some(previous_manifest) = manifest else {
        return Ok(BTreeMap::new());
    };

    let mut entries = BTreeMap::new();
    for entry in &previous_manifest.entries {
        let path = entry.path.as_str();
        if entries.insert(path, entry).is_some() {
            return Err(InstallError::InvalidInstallManifest {
                path: repository_root
                    .join(manifest_path.as_path())
                    .display()
                    .to_string(),
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
    previous_entries_by_path: &BTreeMap<&str, &ManifestEntry>,
) -> Result<Vec<PlannedAssetAction>, InstallError> {
    let mut actions = Vec::with_capacity(assets.len());

    for (asset, manifest_entry) in assets.iter().zip(manifest_entries) {
        let absolute_path = repository_root.join(asset.destination_path.as_path());
        let previous_entry = previous_entries_by_path.get(asset.destination_path.as_str());
        let planned = plan_asset_write(
            &absolute_path,
            asset,
            manifest_entry,
            previous_entry.copied(),
        )?;
        actions.push(planned);
    }

    Ok(actions)
}

fn plan_asset_write(
    absolute_path: &Path,
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
            content: asset.content,
            kind,
        }));
    }

    let current = fs::read(absolute_path).map_err(|err| InstallError::ReadFailure {
        path: absolute_path.display().to_string(),
        message: err.to_string(),
    })?;
    let current_hash = sha256_hex(&current);

    if current_hash == manifest_entry.content_hash {
        return Ok(PlannedAssetAction::Unchanged);
    }

    if asset.preservation == PreservationPolicy::PreserveUserEdits
        && previous_entry.is_some_and(|entry| current_hash != entry.content_hash)
    {
        return Ok(PlannedAssetAction::Preserve(asset.destination_path.clone()));
    }

    Ok(PlannedAssetAction::Write(PlannedWrite {
        path: asset.destination_path.clone(),
        content: asset.content,
        kind: PlannedWriteKind::Updated,
    }))
}

fn collect_preserved_paths(actions: &[PlannedAssetAction]) -> Vec<RepoRelativePath> {
    let mut preserved = Vec::new();
    for action in actions {
        if let PlannedAssetAction::Preserve(path) = action {
            preserved.push(path.clone());
        }
    }
    preserved
}

fn build_removals(
    repository_root: &Path,
    desired_paths: &BTreeSet<String>,
    previous_manifest: Option<&InstallManifest>,
    manifest_path: &RepoRelativePath,
) -> Result<Vec<RepoRelativePath>, InstallError> {
    let Some(manifest) = previous_manifest else {
        return Ok(Vec::new());
    };

    let mut removals = Vec::new();
    for entry in &manifest.entries {
        let path = entry.path.as_str();
        if desired_paths.contains(path)
            || entry.preservation != PreservationPolicy::ReplaceGenerated
        {
            continue;
        }

        let absolute = repository_root.join(entry.path.as_path());
        if absolute.exists() {
            removals.push(entry.path.clone());
        }
    }

    removals.sort();
    ensure_removals_unique(&removals, repository_root, manifest_path)?;
    Ok(removals)
}

fn ensure_removals_unique(
    removals: &[RepoRelativePath],
    repository_root: &Path,
    manifest_path: &RepoRelativePath,
) -> Result<(), InstallError> {
    let mut seen = BTreeSet::new();
    for path in removals {
        if !seen.insert(path.as_str()) {
            return Err(InstallError::InvalidInstallManifest {
                path: repository_root
                    .join(manifest_path.as_path())
                    .display()
                    .to_string(),
                message: format!("duplicate stale removal path '{}'", path.as_str()),
            });
        }
    }
    Ok(())
}
