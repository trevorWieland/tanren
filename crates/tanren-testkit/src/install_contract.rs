//! Typed install-proof contract helpers shared by BDD assertions.

use std::collections::BTreeSet;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
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
