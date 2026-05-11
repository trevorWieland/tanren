//! Typed install-proof contract helpers shared by BDD assertions.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

// Re-export canonical types from the shared contract crate.
// The InstallProof* aliases preserve the public API names used by BDD
// consumers while pointing to the single definition site.
pub use tanren_contract::install::{
    AssetClass as InstallProofAssetClass, InstallContractError as InstallProofContractError,
    InstallIntegration as InstallProofIntegration, InstallProfile as InstallProofProfile,
    RepoRelativePath as InstallProofRepoRelativePath, Sha256Hex, sha256_file_streaming, sha256_hex,
};

/// Install manifest schema version asserted by the BDD install proofs.
pub const INSTALL_MANIFEST_VERSION: u32 = tanren_contract::install::INSTALL_MANIFEST_VERSION;
/// Repo-relative install manifest location asserted by the BDD install proofs.
pub const INSTALL_MANIFEST_REPO_PATH: &str = tanren_contract::install::INSTALL_MANIFEST_REPO_PATH;
/// Rust standards profile identifier for install proofs.
pub const RUST_CARGO_PROFILE_ROOT: &str = "profiles/rust-cargo/";

/// Delivery-owned proof failures surfaced to BDD assertion mapping.
#[derive(Debug, Error)]
pub enum InstallProofError {
    #[error("invalid integration assertion selection '{selection}': {source}")]
    InvalidIntegrationSelection {
        selection: String,
        source: InstallProofContractError,
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

/// Install manifest alias matching the canonical contract type.
pub(crate) use tanren_contract::install::InstallManifest;

#[derive(Debug)]
struct ObservedInstallManifest {
    path: PathBuf,
    raw: String,
    parsed: InstallManifest,
}

/// Assert the default rust-cargo install writes both command and standards assets.
pub fn assert_rust_cargo_default_assets_installed(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    proof::assert_rust_cargo_default_assets_installed_inner(repository_root)
}

/// Assert rust-cargo standards profile assets are installed.
pub fn assert_rust_cargo_standards_installed(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    proof::assert_rust_cargo_standards_installed_inner(repository_root)
}

/// Assert only the selected integration command assets are installed.
pub fn assert_selected_integration_command_assets(
    repository_root: &Path,
    selected_integrations: &str,
) -> Result<(), InstallProofError> {
    proof::assert_selected_integration_command_assets_inner(repository_root, selected_integrations)
}

/// Assert install manifest defaults for rust-cargo profile installs.
pub fn assert_manifest_rust_cargo_defaults(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    proof::assert_manifest_rust_cargo_defaults_inner(repository_root)
}

/// Append a stale generated-manifest row for mutation-flow fixtures.
#[cfg(feature = "test-hooks")]
pub fn append_stale_generated_manifest_entry(
    manifest: &mut String,
    relative_path: &InstallProofRepoRelativePath,
    content_hash: &str,
) {
    let _ = write!(
        manifest,
        "\n[[entries]]\npath = \"{}\"\ncontent_hash = \"{}\"\nasset_class = \"methodology-command\"\nintegration = \"{}\"\npreservation = \"replace-generated\"\n",
        relative_path.as_str(),
        content_hash,
        InstallProofIntegration::Codex.as_str()
    );
}

/// Inject a raw stale generated-manifest row (used by traversal tamper witnesses).
#[cfg(feature = "test-hooks")]
pub fn tamper_manifest_with_raw_generated_entry(
    repository_root: &Path,
    raw_path: &str,
) -> Result<(), InstallProofError> {
    proof::tamper_manifest_with_raw_generated_entry_inner(repository_root, raw_path)
}

/// Read a workspace catalog file for fixture seeding.
#[cfg(feature = "test-hooks")]
pub fn read_workspace_catalog_file(
    relative_path: &InstallProofRepoRelativePath,
) -> Result<String, InstallProofError> {
    proof::read_workspace_catalog_file_inner(relative_path)
}

/// Calculate a hex SHA-256 digest for fixture bytes.
#[must_use]
#[cfg(feature = "test-hooks")]
pub fn sha256_hex_string(bytes: &[u8]) -> String {
    sha256_hex(bytes).to_string()
}

mod proof;
