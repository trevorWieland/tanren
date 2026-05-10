//! Typed install-proof contract helpers shared by BDD assertions.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tanren_cli_app::install::contract;
use tanren_cli_app::install::{RepoRelativePath, sha256_hex};
use thiserror::Error;

pub use tanren_cli_app::install::{
    INSTALL_MANIFEST_REPO_PATH, INSTALL_MANIFEST_VERSION,
    InstallManifestAssetClass as InstallProofAssetClass,
    InstallManifestPreservationPolicy as InstallProofPreservationPolicy,
};

/// Rust standards profile identifier for install proofs.
pub const RUST_CARGO_PROFILE_ROOT: &str = "profiles/rust-cargo/";

/// Profile identifiers supported by install proofs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallProofProfile {
    RustCargo,
}

impl InstallProofProfile {
    /// Canonical profile identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustCargo => "rust-cargo",
        }
    }
}

impl FromStr for InstallProofProfile {
    type Err = InstallProofContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "rust-cargo" => Ok(Self::RustCargo),
            unknown => Err(InstallProofContractError::UnsupportedProfile {
                name: unknown.to_owned(),
            }),
        }
    }
}

/// Integration identifiers supported by install proofs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallProofIntegration {
    Claude,
    Codex,
    OpenCode,
}

impl InstallProofIntegration {
    /// Canonical integration identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "open-code",
        }
    }

    /// Destination root for generated command assets.
    #[must_use]
    pub const fn destination_root(self) -> &'static str {
        match self {
            Self::Claude => ".claude/commands/",
            Self::Codex => ".codex/skills/",
            Self::OpenCode => ".opencode/commands/",
        }
    }

    /// All supported integration identifiers.
    #[must_use]
    pub fn all() -> BTreeSet<Self> {
        [Self::Claude, Self::Codex, Self::OpenCode]
            .into_iter()
            .collect()
    }
}

impl FromStr for InstallProofIntegration {
    type Err = InstallProofContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "open-code" | "opencode" => Ok(Self::OpenCode),
            unknown => Err(InstallProofContractError::UnsupportedIntegration {
                name: unknown.to_owned(),
            }),
        }
    }
}

/// Parse a comma-separated integration selection into typed identifiers.
pub fn parse_install_integration_selection(
    selection: &str,
) -> Result<BTreeSet<InstallProofIntegration>, InstallProofContractError> {
    let mut selected = BTreeSet::new();
    for raw_token in selection.split(',') {
        let token = raw_token.trim();
        if token.is_empty() {
            return Err(InstallProofContractError::EmptyIntegrationSelection);
        }
        selected.insert(token.parse()?);
    }
    if selected.is_empty() {
        return Err(InstallProofContractError::EmptyIntegrationSelection);
    }
    Ok(selected)
}

/// Errors raised while parsing typed install-proof identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InstallProofContractError {
    #[error("unsupported install proof profile '{name}'")]
    UnsupportedProfile { name: String },
    #[error("unsupported install proof integration '{name}'")]
    UnsupportedIntegration { name: String },
    #[error("integration selection is empty")]
    EmptyIntegrationSelection,
}

/// Delivery-owned proof failure type surfaced to BDD assertion mapping.
pub use contract::InstallProofError;
/// Delivery-owned repository-relative install path contract type.
pub type InstallProofRepoRelativePath = RepoRelativePath;

/// Assert the default rust-cargo install writes both command and standards assets.
pub fn assert_rust_cargo_default_assets_installed(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    contract::assert_rust_cargo_default_assets_installed(repository_root)
}

/// Assert rust-cargo standards profile assets are installed.
pub fn assert_rust_cargo_standards_installed(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    contract::assert_rust_cargo_standards_installed(repository_root)
}

/// Assert only the selected integration command assets are installed.
pub fn assert_selected_integration_command_assets(
    repository_root: &Path,
    selected_integrations: &str,
) -> Result<(), InstallProofError> {
    contract::assert_selected_integration_command_assets(repository_root, selected_integrations)
}

/// Assert install manifest defaults for rust-cargo profile installs.
pub fn assert_manifest_rust_cargo_defaults(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    contract::assert_manifest_rust_cargo_defaults(repository_root)
}

/// Assert confirmed uninstall removes generated command assets and install metadata.
pub fn assert_uninstall_removes_generated_assets_and_manifest(
    repository_root: &Path,
) -> Result<(), InstallProofError> {
    contract::assert_uninstall_removes_generated_assets_and_manifest(repository_root)
}

/// Assert uninstall flow preserved a repository file's recorded baseline bytes.
pub fn assert_uninstall_preserves_baseline_file_content(
    repository_root: &Path,
    relative_path: &InstallProofRepoRelativePath,
    baseline: &[u8],
) -> Result<(), InstallProofError> {
    contract::assert_uninstall_preserves_baseline_file_content(
        repository_root,
        relative_path,
        baseline,
    )
}

/// Append a stale generated-manifest row for mutation-flow fixtures.
#[cfg(feature = "test-hooks")]
pub fn append_uninstall_stale_generated_manifest_entry(
    manifest: &mut String,
    relative_path: &InstallProofRepoRelativePath,
    content_hash: &str,
) {
    contract::append_uninstall_stale_generated_manifest_entry(
        manifest,
        relative_path,
        content_hash,
    );
}

/// Inject a raw stale generated-manifest row (used by traversal tamper witnesses).
#[cfg(feature = "test-hooks")]
pub fn tamper_uninstall_manifest_with_raw_generated_entry(
    repository_root: &Path,
    raw_path: &str,
) -> Result<(), InstallProofError> {
    contract::tamper_uninstall_manifest_with_raw_generated_entry(repository_root, raw_path)
}

/// Read a workspace catalog file for fixture seeding.
#[cfg(feature = "test-hooks")]
pub fn read_workspace_catalog_file(
    relative_path: &InstallProofRepoRelativePath,
) -> Result<String, InstallProofError> {
    contract::read_workspace_catalog_file(relative_path)
}

/// Calculate a hex SHA-256 digest for fixture bytes.
#[must_use]
#[cfg(feature = "test-hooks")]
pub fn sha256_hex_string(bytes: &[u8]) -> String {
    sha256_hex(bytes).to_string()
}
