//! Delivery-owned install proof contract helpers used by behavior witnesses.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::install::catalog::generated_integration_destination_roots;
use crate::install::manifest::{
    AssetClass, INSTALL_MANIFEST_REPO_PATH, INSTALL_MANIFEST_VERSION, InstallManifest,
    RepoRelativePath,
};
use crate::install::{
    InstallError, InstallIntegration, InstallProfile, parse_integration_selection,
};

const RUST_CARGO_PROFILE_ROOT: &str = "profiles/rust-cargo/";
mod snapshot;
pub use snapshot::{
    InstallProofRepositorySnapshot, assert_repository_snapshot_matches_baseline,
    assert_uninstall_no_install_keeps_repository_snapshot_unchanged,
    assert_uninstall_preview_keeps_repository_snapshot_unchanged, capture_repository_snapshot,
};

#[cfg(feature = "test-hooks")]
mod test_hooks;
#[cfg(feature = "test-hooks")]
pub use test_hooks::{
    append_uninstall_stale_generated_manifest_entry, read_workspace_catalog_file,
    tamper_uninstall_manifest_with_raw_generated_entry,
};

/// Delivery-owned proof failures surfaced to BDD assertion mapping.
#[derive(Debug, Error)]
pub enum InstallProofError {
    #[error("invalid integration assertion selection '{selection}': {source}")]
    InvalidIntegrationSelection {
        selection: String,
        source: InstallError,
    },
    #[error("failed to canonicalize workspace root while {action}: {source}")]
    CanonicalizeWorkspaceRoot {
        action: &'static str,
        source: std::io::Error,
    },
    #[error("failed to read file '{path}' while {action}: {source}")]
    ReadFile {
        path: PathBuf,
        action: &'static str,
        source: std::io::Error,
    },
    #[error("failed to write file '{path}' while {action}: {source}")]
    WriteFile {
        path: PathBuf,
        action: &'static str,
        source: std::io::Error,
    },
    #[error("failed to read directory '{path}' while {action}: {source}")]
    ReadDirectory {
        path: PathBuf,
        action: &'static str,
        source: std::io::Error,
    },
    #[error("failed to inspect directory entry under '{path}' while {action}: {source}")]
    ReadDirectoryEntry {
        path: PathBuf,
        action: &'static str,
        source: std::io::Error,
    },
    #[error("failed to inspect file type for '{path}' while {action}: {source}")]
    InspectFileType {
        path: PathBuf,
        action: &'static str,
        source: std::io::Error,
    },
    #[error("failed to derive repository-relative snapshot path for '{path}': {source}")]
    SnapshotPathOutsideRoot {
        path: PathBuf,
        source: std::path::StripPrefixError,
    },
    #[error("snapshot path '{path}' violated repository-relative path contract: {source}")]
    SnapshotPathContractRejected { path: String, source: InstallError },
    #[error("failed to parse install manifest '{manifest_path}' as TOML: {source}")]
    InstallManifestTomlParse {
        manifest_path: PathBuf,
        source: toml::de::Error,
    },
    #[error("expected repository file to exist: {path}")]
    ExpectedFileToExist { path: PathBuf },
    #[error("expected repository path to be absent: {path}")]
    ExpectedFileToBeAbsent { path: PathBuf },
    #[error("expected repository file content to match recorded baseline: {path}")]
    ExpectedFileContentToMatchBaseline { path: PathBuf },
    #[error("expected repository snapshot to match recorded baseline")]
    ExpectedRepositorySnapshotToMatchBaseline,
    #[error("expected fixture path to be absent before manifest injection: {path}")]
    StaleManifestPathAlreadyPresent { path: String },
    #[error(
        "install manifest '{manifest_path}' violated proof contract: {expected}\nmanifest:\n{manifest}"
    )]
    ManifestContractViolation {
        expected: String,
        manifest_path: PathBuf,
        manifest: String,
    },
}

/// Assert the default rust-cargo install writes both command and standards assets.
pub fn assert_rust_cargo_default_assets_installed(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    let manifest = read_install_manifest(repository_root)?;
    let expected_integrations = InstallIntegration::all();
    assert_manifest_profile_and_integrations(
        &manifest,
        InstallProfile::RustCargo,
        &expected_integrations,
    )?;
    assert_command_assets_for_selected_integrations(
        repository_root,
        &manifest,
        &expected_integrations,
    )?;
    assert_standards_assets_for_rust_cargo(repository_root, &manifest)?;
    Ok(())
}

/// Assert rust-cargo standards profile assets are installed.
pub fn assert_rust_cargo_standards_installed(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    let manifest = read_install_manifest(repository_root)?;
    assert_standards_assets_for_rust_cargo(repository_root, &manifest)
}

/// Assert only the selected integration command assets are installed.
pub fn assert_selected_integration_command_assets(
    repository_root: &Path,
    selected_integrations: &str,
) -> Result<(), InstallProofError> {
    let selected = parse_integration_selection(Some(selected_integrations)).map_err(|source| {
        InstallProofError::InvalidIntegrationSelection {
            selection: selected_integrations.to_owned(),
            source,
        }
    })?;
    let manifest = read_install_manifest(repository_root)?;
    assert_manifest_profile_and_integrations(&manifest, InstallProfile::RustCargo, &selected)?;
    assert_command_assets_for_selected_integrations(repository_root, &manifest, &selected)?;
    assert_unselected_integration_roots_are_empty(repository_root, &selected)
}

/// Assert install manifest defaults for rust-cargo profile installs.
pub fn assert_manifest_rust_cargo_defaults(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    let manifest = read_install_manifest(repository_root)?;
    let expected_integrations = InstallIntegration::all();
    assert_manifest_profile_and_integrations(
        &manifest,
        InstallProfile::RustCargo,
        &expected_integrations,
    )?;

    let has_command_assets = manifest
        .parsed
        .entries
        .iter()
        .any(|entry| entry.asset_class == AssetClass::MethodologyCommand);
    if !has_command_assets {
        return Err(manifest_contract_error(
            &manifest,
            "manifest must include at least one methodology-command entry",
        ));
    }

    let has_standards_assets = manifest
        .parsed
        .entries
        .iter()
        .any(|entry| entry.asset_class == AssetClass::StandardsProfile);
    if !has_standards_assets {
        return Err(manifest_contract_error(
            &manifest,
            "manifest must include at least one standards-profile entry",
        ));
    }

    Ok(())
}
pub fn assert_uninstall_removes_generated_assets_and_manifest(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    let manifest_path = repository_root.join(INSTALL_MANIFEST_REPO_PATH);
    if manifest_path.exists() {
        return Err(InstallProofError::ExpectedFileToBeAbsent {
            path: manifest_path,
        });
    }
    assert_unselected_integration_roots_are_empty(repository_root, &BTreeSet::new())
}

/// Assert a repository path preserves recorded baseline bytes after uninstall flows.
pub fn assert_uninstall_preserves_baseline_file_content(
    repository_root: &Path,
    relative_path: &RepoRelativePath,
    baseline: &[u8],
) -> Result<(), InstallProofError> {
    let absolute_path = repository_root.join(relative_path.as_str());
    let observed_bytes =
        fs::read(&absolute_path).map_err(|source| InstallProofError::ReadFile {
            path: absolute_path.clone(),
            action: "read uninstall-preserved repository fixture file",
            source,
        })?;
    if observed_bytes != baseline {
        return Err(InstallProofError::ExpectedFileContentToMatchBaseline {
            path: absolute_path,
        });
    }
    Ok(())
}

#[derive(Debug)]
struct ObservedInstallManifest {
    path: PathBuf,
    raw: String,
    parsed: InstallManifest,
}

fn read_install_manifest(
    repository_root: &Path,
) -> Result<ObservedInstallManifest, InstallProofError> {
    let path = repository_root.join(INSTALL_MANIFEST_REPO_PATH);
    let raw = read_to_string_with_context(&path, "read install manifest from repository fixture")?;
    let parsed: InstallManifest =
        toml::from_str(&raw).map_err(|source| InstallProofError::InstallManifestTomlParse {
            manifest_path: path.clone(),
            source,
        })?;

    if parsed.manifest_version != INSTALL_MANIFEST_VERSION {
        let actual_version = parsed.manifest_version;
        return Err(manifest_contract_error(
            &ObservedInstallManifest { path, raw, parsed },
            &format!(
                "manifest_version must equal {INSTALL_MANIFEST_VERSION}, got {actual_version}"
            ),
        ));
    }

    Ok(ObservedInstallManifest { path, raw, parsed })
}

fn assert_manifest_profile_and_integrations(
    manifest: &ObservedInstallManifest,
    expected_profile: InstallProfile,
    expected_integrations: &BTreeSet<InstallIntegration>,
) -> Result<(), InstallProofError> {
    if manifest.parsed.profile != expected_profile {
        return Err(manifest_contract_error(
            manifest,
            &format!(
                "manifest profile must be '{}', got '{}'",
                expected_profile.as_str(),
                manifest.parsed.profile.as_str()
            ),
        ));
    }

    let actual_integrations: BTreeSet<InstallIntegration> =
        manifest.parsed.integrations.iter().copied().collect();
    if actual_integrations != *expected_integrations {
        return Err(manifest_contract_error(
            manifest,
            &format!(
                "manifest integrations must be {}, got {}",
                format_integration_set(expected_integrations),
                format_integration_set(&actual_integrations)
            ),
        ));
    }

    Ok(())
}

fn assert_command_assets_for_selected_integrations(
    repository_root: &Path,
    manifest: &ObservedInstallManifest,
    selected: &BTreeSet<InstallIntegration>,
) -> Result<(), InstallProofError> {
    let mut seen = BTreeSet::new();
    for entry in manifest
        .parsed
        .entries
        .iter()
        .filter(|entry| entry.asset_class == AssetClass::MethodologyCommand)
    {
        let integration = entry.integration.ok_or_else(|| {
            manifest_contract_error(
                manifest,
                &format!(
                    "methodology-command entry '{}' must include integration",
                    entry.path.as_str()
                ),
            )
        })?;

        if !selected.contains(&integration) {
            return Err(manifest_contract_error(
                manifest,
                &format!(
                    "methodology-command entry '{}' unexpectedly targets integration '{}'",
                    entry.path.as_str(),
                    integration.as_str()
                ),
            ));
        }

        let destination_roots =
            generated_integration_destination_roots(&BTreeSet::from([integration]));
        if !super::catalog::is_current_generated_integration_destination(
            &entry.path,
            &destination_roots,
        ) {
            return Err(manifest_contract_error(
                manifest,
                &format!(
                    "methodology-command entry '{}' must be under selected integration destination root",
                    entry.path.as_str()
                ),
            ));
        }

        assert_file_exists(repository_root, entry.path.as_str())?;
        seen.insert(integration);
    }

    if seen != *selected {
        return Err(manifest_contract_error(
            manifest,
            &format!(
                "methodology-command integrations must match selection {}; saw {}",
                format_integration_set(selected),
                format_integration_set(&seen)
            ),
        ));
    }

    Ok(())
}

fn assert_standards_assets_for_rust_cargo(
    repository_root: &Path,
    manifest: &ObservedInstallManifest,
) -> Result<(), InstallProofError> {
    let mut saw_any = false;
    for entry in manifest
        .parsed
        .entries
        .iter()
        .filter(|entry| entry.asset_class == AssetClass::StandardsProfile)
    {
        saw_any = true;
        if !entry.path.as_str().starts_with(RUST_CARGO_PROFILE_ROOT) {
            return Err(manifest_contract_error(
                manifest,
                &format!(
                    "standards-profile entry '{}' must be under '{}'",
                    entry.path.as_str(),
                    RUST_CARGO_PROFILE_ROOT
                ),
            ));
        }
        assert_file_exists(repository_root, entry.path.as_str())?;
    }

    if !saw_any {
        return Err(manifest_contract_error(
            manifest,
            "manifest must include at least one standards-profile entry",
        ));
    }

    Ok(())
}

fn assert_unselected_integration_roots_are_empty(
    repository_root: &Path,
    selected: &BTreeSet<InstallIntegration>,
) -> Result<(), InstallProofError> {
    for integration in InstallIntegration::all() {
        if selected.contains(&integration) {
            continue;
        }

        let roots = generated_integration_destination_roots(&BTreeSet::from([integration]));
        for root in roots {
            let root_path = repository_root.join(root);
            if has_any_files(&root_path)? {
                return Err(InstallProofError::ExpectedFileToBeAbsent { path: root_path });
            }
        }
    }

    Ok(())
}

fn has_any_files(path: &Path) -> Result<bool, InstallProofError> {
    if !path.exists() {
        return Ok(false);
    }

    let entries = fs::read_dir(path).map_err(|source| InstallProofError::ReadDirectory {
        path: path.to_path_buf(),
        action: "inspect unselected integration root",
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| InstallProofError::ReadDirectoryEntry {
            path: path.to_path_buf(),
            action: "inspect unselected integration root entries",
            source,
        })?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| InstallProofError::InspectFileType {
                path: entry_path.clone(),
                action: "inspect unselected integration root entry type",
                source,
            })?;

        if file_type.is_file() {
            return Ok(true);
        }
        if file_type.is_dir() && has_any_files(&entry_path)? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn manifest_contract_error(
    manifest: &ObservedInstallManifest,
    expected: &str,
) -> InstallProofError {
    InstallProofError::ManifestContractViolation {
        expected: expected.to_owned(),
        manifest_path: manifest.path.clone(),
        manifest: manifest.raw.clone(),
    }
}

fn format_integration_set(set: &BTreeSet<InstallIntegration>) -> String {
    format!(
        "[{}]",
        set.iter()
            .map(|integration| integration.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn assert_file_exists(
    repository_root: &Path,
    relative_path: &str,
) -> Result<(), InstallProofError> {
    let absolute = repository_root.join(relative_path);
    absolute
        .exists()
        .then_some(())
        .ok_or(InstallProofError::ExpectedFileToExist { path: absolute })
}
fn read_to_string_with_context(
    path: &Path,
    action: &'static str,
) -> Result<String, InstallProofError> {
    fs::read_to_string(path).map_err(|source| InstallProofError::ReadFile {
        path: path.to_path_buf(),
        action,
        source,
    })
}
