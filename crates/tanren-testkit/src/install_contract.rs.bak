//! Typed install-proof contract helpers shared by BDD assertions.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tanren_cli_app::install::contract;
use tanren_cli_app::install::{RepoRelativePath, sha256_hex};
use thiserror::Error;

/// Install manifest schema version asserted by the BDD install proofs.
pub const INSTALL_MANIFEST_VERSION: u32 = 1;
/// Repo-relative install manifest location asserted by the BDD install proofs.
pub const INSTALL_MANIFEST_REPO_PATH: &str = ".tanren/install-manifest.toml";
/// Repo-relative project methodology config projection location.
pub const PROJECT_METHODOLOGY_CONFIG_REPO_PATH: &str = ".tanren/project-methodology.toml";
/// Rust standards profile identifier for install proofs.
pub const RUST_CARGO_PROFILE_ROOT: &str = "profiles/rust-cargo/";
/// Default standards root configured by rust-cargo install profile.
pub const RUST_CARGO_STANDARDS_ROOT: &str = "profiles/rust-cargo";

/// Typed effective-configuration read-model fixture for BDD assertions.
///
/// Represents the resolved effective configuration that standards inspect
/// should report. Seeded by the BDD context from known inputs (profile and
/// standards root), not by loading `.tanren/project-methodology.toml` as
/// the canonical expected source. The repo file is a non-secret generated
/// projection; the fixture is the authority for expected values in
/// standards-inspect report assertions.
#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveConfigurationFixture {
    profile: InstallProofProfile,
    standards_root: String,
}

#[cfg(feature = "test-hooks")]
impl EffectiveConfigurationFixture {
    /// Build a fixture from the install profile and configured standards root.
    #[must_use]
    pub fn new(profile: InstallProofProfile, standards_root: &str) -> Self {
        Self {
            profile,
            standards_root: standards_root.to_owned(),
        }
    }

    /// The expected methodology profile.
    #[must_use]
    pub fn profile(&self) -> InstallProofProfile {
        self.profile
    }

    /// The expected profile identifier string.
    #[must_use]
    pub fn profile_str(&self) -> &'static str {
        self.profile.as_str()
    }

    /// The expected standards root path.
    #[must_use]
    pub fn standards_root(&self) -> &str {
        &self.standards_root
    }
}
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

/// Install manifest asset-class identifiers used by BDD assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallProofAssetClass {
    MethodologyCommand,
    StandardsProfile,
    MethodologyConfig,
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

/// Append a stale generated-manifest row for mutation-flow fixtures.
#[cfg(feature = "test-hooks")]
pub fn append_stale_generated_manifest_entry(
    manifest: &mut String,
    relative_path: &InstallProofRepoRelativePath,
    content_hash: &str,
) {
    contract::append_stale_generated_manifest_entry(manifest, relative_path, content_hash);
}

/// Inject a raw stale generated-manifest row (used by traversal tamper witnesses).
#[cfg(feature = "test-hooks")]
pub fn tamper_manifest_with_raw_generated_entry(
    repository_root: &Path,
    raw_path: &str,
) -> Result<(), InstallProofError> {
    contract::tamper_manifest_with_raw_generated_entry(repository_root, raw_path)
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
