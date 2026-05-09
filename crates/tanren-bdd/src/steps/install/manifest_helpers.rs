use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tanren_testkit::{
    INSTALL_MANIFEST_REPO_PATH, INSTALL_MANIFEST_VERSION, InstallProofAssetClass,
    InstallProofIntegration, InstallProofProfile, RUST_CARGO_PROFILE_ROOT,
    parse_install_integration_selection,
};

use super::InstallStepError;

const NIBBLES: &[u8; 16] = b"0123456789abcdef";
const TAMPERED_ENTRY_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

pub(crate) fn assert_rust_cargo_default_assets_installed(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    let manifest = read_install_manifest(repository_root)?;
    let expected_integrations = InstallProofIntegration::all();
    assert_manifest_profile_and_integrations(
        &manifest,
        InstallProofProfile::RustCargo,
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

pub(crate) fn assert_rust_cargo_standards_installed(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    let manifest = read_install_manifest(repository_root)?;
    assert_standards_assets_for_rust_cargo(repository_root, &manifest)
}

pub(crate) fn assert_selected_integration_command_assets(
    repository_root: &Path,
    selected_integrations: &str,
) -> Result<(), InstallStepError> {
    let selected =
        parse_install_integration_selection(selected_integrations).map_err(|source| {
            InstallStepError::InvalidIntegrationSelectionForAssertion {
                selection: selected_integrations.to_owned(),
                source,
            }
        })?;
    let manifest = read_install_manifest(repository_root)?;
    assert_manifest_profile_and_integrations(&manifest, InstallProofProfile::RustCargo, &selected)?;
    assert_command_assets_for_selected_integrations(repository_root, &manifest, &selected)?;
    assert_unselected_integration_roots_are_empty(repository_root, &selected)
}

pub(crate) fn assert_manifest_rust_cargo_defaults(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    let manifest = read_install_manifest(repository_root)?;
    let expected_integrations = InstallProofIntegration::all();
    assert_manifest_profile_and_integrations(
        &manifest,
        InstallProofProfile::RustCargo,
        &expected_integrations,
    )?;

    let has_command_assets = manifest
        .entries
        .iter()
        .any(|entry| entry.asset_class == InstallProofAssetClass::MethodologyCommand);
    if !has_command_assets {
        return Err(manifest_contract_error(
            &manifest,
            "manifest must include at least one methodology-command entry",
        ));
    }

    let has_standards_assets = manifest
        .entries
        .iter()
        .any(|entry| entry.asset_class == InstallProofAssetClass::StandardsProfile);
    if !has_standards_assets {
        return Err(manifest_contract_error(
            &manifest,
            "manifest must include at least one standards-profile entry",
        ));
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RepositoryRelativePath(String);

impl RepositoryRelativePath {
    pub(crate) fn parse(raw: String) -> Result<Self, InstallStepError> {
        validate_relative_path(raw.as_str())?;
        Ok(Self(raw))
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

pub(crate) fn validate_relative_path(path: &str) -> Result<(), InstallStepError> {
    if path.is_empty() {
        return Err(InstallStepError::EmptyRepositoryRelativePath);
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err(InstallStepError::AbsoluteRepositoryRelativePath {
            path: path.to_owned(),
        });
    }
    let is_valid = candidate
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
    if !is_valid {
        return Err(InstallStepError::TraversalRepositoryRelativePath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn append_stale_generated_manifest_entry(
    manifest: &mut String,
    relative_path: &RepositoryRelativePath,
    content_hash: String,
) {
    let entry =
        GeneratedManifestEntry::stale_methodology_command(relative_path.clone(), content_hash);
    manifest.push_str(&entry.to_manifest_block());
}

pub(crate) fn tamper_manifest_with_raw_generated_entry(
    repository_root: &Path,
    raw_path: &str,
) -> Result<(), InstallStepError> {
    let manifest_path = repository_root.join(INSTALL_MANIFEST_REPO_PATH);
    let mut manifest = read_to_string_with_context(&manifest_path, "read install manifest")?;
    let path_line = format!("path = \"{raw_path}\"");
    if manifest.contains(&path_line) {
        return Err(InstallStepError::StaleManifestPathAlreadyPresent {
            path: raw_path.to_owned(),
        });
    }
    let _ = write!(
        manifest,
        "\n[[entries]]\npath = \"{raw_path}\"\ncontent_hash = \"{TAMPERED_ENTRY_SHA256}\"\nasset_class = \"methodology-command\"\nintegration = \"{}\"\npreservation = \"replace-generated\"\n",
        InstallProofIntegration::Codex.as_str()
    );
    fs::write(&manifest_path, manifest).map_err(|source| InstallStepError::WriteFile {
        path: manifest_path,
        action: "write install manifest with tampered raw entry",
        source,
    })?;
    Ok(())
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(char::from(NIBBLES[(byte >> 4) as usize]));
        hex.push(char::from(NIBBLES[(byte & 0x0f) as usize]));
    }
    hex
}

pub(crate) fn read_workspace_catalog_file(relative_path: &str) -> Result<String, InstallStepError> {
    validate_relative_path(relative_path)?;
    let absolute = workspace_root()?.join(relative_path);
    read_to_string_with_context(&absolute, "read workspace catalog file")
}

#[derive(Debug)]
struct ObservedInstallManifest {
    path: PathBuf,
    raw: String,
    profile: InstallProofProfile,
    integrations: BTreeSet<InstallProofIntegration>,
    entries: Vec<ObservedInstallManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct ObservedInstallManifestToml {
    manifest_version: u32,
    profile: InstallProofProfile,
    integrations: Vec<InstallProofIntegration>,
    entries: Vec<ObservedInstallManifestEntry>,
}

#[derive(Debug, Deserialize)]
struct ObservedInstallManifestEntry {
    path: String,
    asset_class: InstallProofAssetClass,
    integration: Option<InstallProofIntegration>,
}

fn read_install_manifest(
    repository_root: &Path,
) -> Result<ObservedInstallManifest, InstallStepError> {
    let path = repository_root.join(INSTALL_MANIFEST_REPO_PATH);
    let raw = read_to_string_with_context(&path, "read install manifest from repository fixture")?;
    let parsed: ObservedInstallManifestToml =
        toml::from_str(&raw).map_err(|source| InstallStepError::InstallManifestTomlParse {
            manifest_path: path.clone(),
            source,
        })?;

    if parsed.manifest_version != INSTALL_MANIFEST_VERSION {
        return Err(InstallStepError::ManifestMissingContent {
            expected: format!(
                "manifest_version must equal {INSTALL_MANIFEST_VERSION}, got {}",
                parsed.manifest_version
            ),
            manifest_path: path,
            manifest: raw,
        });
    }

    Ok(ObservedInstallManifest {
        path,
        raw,
        profile: parsed.profile,
        integrations: parsed.integrations.into_iter().collect(),
        entries: parsed.entries,
    })
}

fn assert_manifest_profile_and_integrations(
    manifest: &ObservedInstallManifest,
    expected_profile: InstallProofProfile,
    expected_integrations: &BTreeSet<InstallProofIntegration>,
) -> Result<(), InstallStepError> {
    if manifest.profile != expected_profile {
        return Err(manifest_contract_error(
            manifest,
            &format!(
                "manifest profile must be '{}', got '{}'",
                expected_profile.as_str(),
                manifest.profile.as_str()
            ),
        ));
    }
    if manifest.integrations != *expected_integrations {
        return Err(manifest_contract_error(
            manifest,
            &format!(
                "manifest integrations must be {}, got {}",
                format_integration_set(expected_integrations),
                format_integration_set(&manifest.integrations)
            ),
        ));
    }
    Ok(())
}

fn assert_command_assets_for_selected_integrations(
    repository_root: &Path,
    manifest: &ObservedInstallManifest,
    selected: &BTreeSet<InstallProofIntegration>,
) -> Result<(), InstallStepError> {
    let mut seen = BTreeSet::new();
    for entry in manifest
        .entries
        .iter()
        .filter(|entry| entry.asset_class == InstallProofAssetClass::MethodologyCommand)
    {
        let integration = entry.integration.ok_or_else(|| {
            manifest_contract_error(
                manifest,
                &format!(
                    "methodology-command entry '{}' must include integration",
                    entry.path
                ),
            )
        })?;
        if !selected.contains(&integration) {
            return Err(manifest_contract_error(
                manifest,
                &format!(
                    "methodology-command entry '{}' unexpectedly targets integration '{}'",
                    entry.path,
                    integration.as_str()
                ),
            ));
        }

        validate_relative_path(entry.path.as_str())?;
        if !entry.path.starts_with(integration.destination_root()) {
            return Err(manifest_contract_error(
                manifest,
                &format!(
                    "methodology-command entry '{}' must be under '{}'",
                    entry.path,
                    integration.destination_root()
                ),
            ));
        }

        assert_file_exists(repository_root, &entry.path)?;
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
) -> Result<(), InstallStepError> {
    let mut saw_any = false;
    for entry in manifest
        .entries
        .iter()
        .filter(|entry| entry.asset_class == InstallProofAssetClass::StandardsProfile)
    {
        saw_any = true;
        validate_relative_path(entry.path.as_str())?;
        if !entry.path.starts_with(RUST_CARGO_PROFILE_ROOT) {
            return Err(manifest_contract_error(
                manifest,
                &format!(
                    "standards-profile entry '{}' must be under '{}'",
                    entry.path, RUST_CARGO_PROFILE_ROOT
                ),
            ));
        }
        assert_file_exists(repository_root, &entry.path)?;
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
    selected: &BTreeSet<InstallProofIntegration>,
) -> Result<(), InstallStepError> {
    for integration in InstallProofIntegration::all() {
        if selected.contains(&integration) {
            continue;
        }
        let root = repository_root.join(integration.destination_root());
        if has_any_files(&root)? {
            return Err(InstallStepError::ExpectedFileToBeAbsent { path: root });
        }
    }
    Ok(())
}

fn has_any_files(path: &Path) -> Result<bool, InstallStepError> {
    if !path.exists() {
        return Ok(false);
    }
    let entries = fs::read_dir(path).map_err(|source| InstallStepError::ReadDirectory {
        path: path.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| InstallStepError::ReadDirectoryEntry {
            path: path.to_path_buf(),
            source,
        })?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| InstallStepError::InspectFileType {
                path: entry_path.clone(),
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

fn manifest_contract_error(manifest: &ObservedInstallManifest, expected: &str) -> InstallStepError {
    InstallStepError::ManifestMissingContent {
        expected: expected.to_owned(),
        manifest_path: manifest.path.clone(),
        manifest: manifest.raw.clone(),
    }
}

fn format_integration_set(set: &BTreeSet<InstallProofIntegration>) -> String {
    let values = set
        .iter()
        .map(|integration| integration.as_str())
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneratedManifestEntry {
    path: RepositoryRelativePath,
    content_hash: String,
    asset_class: InstallProofAssetClass,
    integration: InstallProofIntegration,
    preservation: &'static str,
}

impl GeneratedManifestEntry {
    fn stale_methodology_command(path: RepositoryRelativePath, content_hash: String) -> Self {
        Self {
            path,
            content_hash,
            asset_class: InstallProofAssetClass::MethodologyCommand,
            integration: InstallProofIntegration::Codex,
            preservation: "replace-generated",
        }
    }

    fn to_manifest_block(&self) -> String {
        let asset_class = match self.asset_class {
            InstallProofAssetClass::MethodologyCommand => "methodology-command",
            InstallProofAssetClass::StandardsProfile => "standards-profile",
        };
        format!(
            "\n[[entries]]\npath = \"{}\"\ncontent_hash = \"{}\"\nasset_class = \"{}\"\nintegration = \"{}\"\npreservation = \"{}\"\n",
            self.path.as_str(),
            self.content_hash,
            asset_class,
            self.integration.as_str(),
            self.preservation
        )
    }
}

fn workspace_root() -> Result<PathBuf, InstallStepError> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|source| InstallStepError::CanonicalizeWorkspaceRoot { source })
}

fn assert_file_exists(repository_root: &Path, relative_path: &str) -> Result<(), InstallStepError> {
    let absolute = repository_root.join(relative_path);
    if !absolute.exists() {
        return Err(InstallStepError::ExpectedFileToExist { path: absolute });
    }
    Ok(())
}

fn read_to_string_with_context(
    path: &Path,
    action: &'static str,
) -> Result<String, InstallStepError> {
    fs::read_to_string(path).map_err(|source| InstallStepError::ReadFile {
        path: path.to_path_buf(),
        action,
        source,
    })
}

pub(crate) fn io_error(
    path: PathBuf,
    action: &'static str,
    source: std::io::Error,
) -> InstallStepError {
    InstallStepError::Io {
        path,
        action,
        source,
    }
}
