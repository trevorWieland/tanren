//! Typed install-proof contract helpers shared by BDD assertions.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Install manifest schema version asserted by the BDD install proofs.
pub const INSTALL_MANIFEST_VERSION: u32 = 1;
/// Repo-relative install manifest location asserted by the BDD install proofs.
pub const INSTALL_MANIFEST_REPO_PATH: &str = ".tanren/install-manifest.toml";
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

/// Install manifest asset-class identifiers used by BDD assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallProofAssetClass {
    MethodologyCommand,
    StandardsProfile,
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

/// Parse failure for install proof repository-relative paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("install proof path must be a non-empty repository-relative path without traversal")]
pub struct InstallProofRepoRelativePathParseError;

/// Strict repository-relative path for install-proof fixtures.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstallProofRepoRelativePath(String);

impl InstallProofRepoRelativePath {
    /// Validate and construct a repository-relative path.
    pub fn parse(path: &str) -> Result<Self, InstallProofRepoRelativePathParseError> {
        if path.is_empty() {
            return Err(InstallProofRepoRelativePathParseError);
        }
        let candidate = Path::new(path);
        if candidate.is_absolute() {
            return Err(InstallProofRepoRelativePathParseError);
        }
        let is_valid = candidate
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
        if !is_valid {
            return Err(InstallProofRepoRelativePathParseError);
        }
        Ok(Self(path.to_owned()))
    }

    /// Borrow the validated path string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Serialize for InstallProofRepoRelativePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for InstallProofRepoRelativePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        InstallProofRepoRelativePath::parse(raw.as_str()).map_err(serde::de::Error::custom)
    }
}

/// Typed parse failure for [`Sha256Hex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("content hash must be exactly 64 lowercase hex characters")]
struct Sha256HexParseError;

/// Delivery-owned proof failure type surfaced to BDD assertion mapping.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum InstallProofPreservation {
    ReplaceGenerated,
    PreserveUserEdits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Sha256Hex(String);

impl Sha256Hex {
    const LENGTH: usize = 64;

    fn parse(value: &str) -> Result<Self, Sha256HexParseError> {
        if value.len() != Self::LENGTH
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Sha256HexParseError);
        }
        Ok(Self(value.to_owned()))
    }

    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Serialize for Sha256Hex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Sha256Hex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Sha256Hex::parse(raw.as_str()).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct InstallManifestEntry {
    path: InstallProofRepoRelativePath,
    content_hash: Sha256Hex,
    asset_class: InstallProofAssetClass,
    integration: Option<InstallProofIntegration>,
    preservation: InstallProofPreservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct InstallManifest {
    manifest_version: u32,
    profile: InstallProofProfile,
    integrations: Vec<InstallProofIntegration>,
    entries: Vec<InstallManifestEntry>,
}

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
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

mod proof;
