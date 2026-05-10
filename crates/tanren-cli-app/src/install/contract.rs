//! Delivery-owned install proof contract helpers used by behavior witnesses.

use std::collections::BTreeSet;
use std::fmt::Write as _;
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

#[path = "contract_fs.rs"]
mod contract_fs;
#[path = "contract_methodology.rs"]
mod contract_methodology;

const TAMPERED_ENTRY_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
const RUST_CARGO_PROFILE_ROOT: &str = "profiles/rust-cargo/";
pub(super) const RUST_CARGO_STANDARDS_ROOT: &str = "profiles/rust-cargo";

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
    #[error("failed to parse install manifest '{manifest_path}' as TOML: {source}")]
    InstallManifestTomlParse {
        manifest_path: PathBuf,
        source: toml::de::Error,
    },
    #[error("failed to parse project methodology config '{path}' as TOML: {source}")]
    ProjectMethodologyConfigTomlParse {
        path: PathBuf,
        source: tanren_configuration_secrets::ConfigSecretsError,
    },
    #[error("expected repository file to exist: {path}")]
    ExpectedFileToExist { path: PathBuf },
    #[error("expected repository path to be absent: {path}")]
    ExpectedFileToBeAbsent { path: PathBuf },
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
    contract_methodology::assert_project_methodology_config_for_rust_cargo(
        repository_root,
        &manifest,
    )?;
    Ok(())
}

/// Assert rust-cargo standards profile assets are installed.
pub fn assert_rust_cargo_standards_installed(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    let manifest = read_install_manifest(repository_root)?;
    assert_standards_assets_for_rust_cargo(repository_root, &manifest)?;
    contract_methodology::assert_project_methodology_config_for_rust_cargo(
        repository_root,
        &manifest,
    )
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

    let has_methodology_config = manifest
        .parsed
        .entries
        .iter()
        .any(|entry| entry.asset_class == AssetClass::MethodologyConfig);
    if !has_methodology_config {
        return Err(manifest_contract_error(
            &manifest,
            "manifest must include one methodology-config entry",
        ));
    }

    Ok(())
}

/// Append a stale generated-manifest row for mutation-flow fixtures.
#[cfg(feature = "test-hooks")]
pub fn append_stale_generated_manifest_entry(
    manifest: &mut String,
    relative_path: &RepoRelativePath,
    content_hash: &str,
) {
    let _ = write!(
        manifest,
        "\n[[entries]]\npath = \"{}\"\ncontent_hash = \"{}\"\nasset_class = \"methodology-command\"\nintegration = \"{}\"\npreservation = \"replace-generated\"\n",
        relative_path.as_str(),
        content_hash,
        InstallIntegration::Codex.as_str()
    );
}

/// Inject a raw stale generated-manifest row (used by traversal tamper witnesses).
#[cfg(feature = "test-hooks")]
pub fn tamper_manifest_with_raw_generated_entry(
    repository_root: &Path,
    raw_path: &str,
) -> Result<(), InstallProofError> {
    let manifest_path = repository_root.join(INSTALL_MANIFEST_REPO_PATH);
    let mut manifest =
        contract_fs::read_to_string_with_context(&manifest_path, "read install manifest")?;
    let path_line = format!("path = \"{raw_path}\"");
    if manifest.contains(&path_line) {
        return Err(InstallProofError::StaleManifestPathAlreadyPresent {
            path: raw_path.to_owned(),
        });
    }
    let _ = write!(
        manifest,
        "\n[[entries]]\npath = \"{raw_path}\"\ncontent_hash = \"{TAMPERED_ENTRY_SHA256}\"\nasset_class = \"methodology-command\"\nintegration = \"{}\"\npreservation = \"replace-generated\"\n",
        InstallIntegration::Codex.as_str()
    );
    fs::write(&manifest_path, manifest).map_err(|source| InstallProofError::WriteFile {
        path: manifest_path,
        action: "write install manifest with tampered raw entry",
        source,
    })?;
    Ok(())
}

/// Read a workspace catalog file for fixture seeding.
#[cfg(feature = "test-hooks")]
pub fn read_workspace_catalog_file(
    relative_path: &RepoRelativePath,
) -> Result<String, InstallProofError> {
    let absolute = contract_fs::workspace_root()?.join(relative_path.as_str());
    contract_fs::read_to_string_with_context(&absolute, "read workspace catalog file")
}

#[derive(Debug)]
pub(super) struct ObservedInstallManifest {
    pub(super) path: PathBuf,
    pub(super) raw: String,
    pub(super) parsed: InstallManifest,
}

fn read_install_manifest(
    repository_root: &Path,
) -> Result<ObservedInstallManifest, InstallProofError> {
    let path = repository_root.join(INSTALL_MANIFEST_REPO_PATH);
    let raw = contract_fs::read_to_string_with_context(
        &path,
        "read install manifest from repository fixture",
    )?;
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

        contract_fs::assert_file_exists(repository_root, entry.path.as_str())?;
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
        contract_fs::assert_file_exists(repository_root, entry.path.as_str())?;
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
    contract_fs::assert_unselected_integration_roots_are_empty(repository_root, selected)
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
    let values = set
        .iter()
        .map(|integration| integration.as_str())
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
}
