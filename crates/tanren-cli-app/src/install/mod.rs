//! Install profile/integration typing and catalog plumbing.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

mod catalog;
mod cli;
#[cfg(feature = "test-hooks")]
pub mod contract;
mod error;
mod manifest;
mod path_guard;
mod plan;
mod uninstall_plan;
mod writer;
mod writer_tx;

pub use cli::{InstallCommand, UninstallCommand};
pub use error::InstallError;
#[cfg(feature = "test-hooks")]
pub use manifest::RepoRelativePath;
#[cfg(feature = "test-hooks")]
pub use manifest::{
    AssetClass as InstallManifestAssetClass,
    PreservationPolicy as InstallManifestPreservationPolicy,
};
#[cfg(feature = "test-hooks")]
pub const INSTALL_MANIFEST_REPO_PATH: &str = manifest::INSTALL_MANIFEST_REPO_PATH;
#[cfg(feature = "test-hooks")]
pub const INSTALL_MANIFEST_VERSION: u32 = manifest::INSTALL_MANIFEST_VERSION;
pub use plan::InstallPlan;
pub use uninstall_plan::{
    UninstallNothingReason, UninstallPreserveReason, UninstallPreservedPath, UninstallPreview,
    UninstallWarning, UninstallWarningKind,
};
pub use writer::{InstallReport, UninstallApplyReport};

/// Calculate a hex SHA-256 digest for test fixture bytes.
#[must_use]
#[cfg(feature = "test-hooks")]
pub fn sha256_hex(bytes: &[u8]) -> manifest::Sha256Hex {
    manifest::sha256_hex(bytes)
}

/// Supported Tanren standards profiles for local repository bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallProfile {
    /// Install the Rust + Cargo standards profile.
    RustCargo,
}

impl InstallProfile {
    /// Canonical profile identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustCargo => "rust-cargo",
        }
    }
}

impl FromStr for InstallProfile {
    type Err = InstallError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "rust-cargo" => Ok(Self::RustCargo),
            unknown => Err(InstallError::UnsupportedProfile {
                name: unknown.to_owned(),
            }),
        }
    }
}

/// Supported agent integration targets for generated command assets.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum InstallIntegration {
    Claude,
    Codex,
    OpenCode,
}

impl InstallIntegration {
    /// Canonical integration identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "open-code",
        }
    }

    /// Return all supported integrations.
    #[must_use]
    pub fn all() -> BTreeSet<Self> {
        [Self::Claude, Self::Codex, Self::OpenCode]
            .into_iter()
            .collect()
    }
}

impl FromStr for InstallIntegration {
    type Err = InstallError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "open-code" | "opencode" => Ok(Self::OpenCode),
            unknown => Err(InstallError::UnsupportedIntegration {
                name: unknown.to_owned(),
            }),
        }
    }
}

/// Parse integration selection from comma-separated CLI values.
///
/// The parser validates all names before install planning. `None` means
/// "install all supported integrations".
pub fn parse_integration_selection(
    selection: Option<&str>,
) -> Result<BTreeSet<InstallIntegration>, InstallError> {
    let Some(raw_selection) = selection else {
        return Ok(InstallIntegration::all());
    };

    let mut selected = BTreeSet::new();
    for raw_token in raw_selection.split(',') {
        let token = raw_token.trim();
        if token.is_empty() {
            return Err(InstallError::EmptyIntegrationSelection);
        }
        selected.insert(token.parse()?);
    }

    if selected.is_empty() {
        return Err(InstallError::EmptyIntegrationSelection);
    }

    Ok(selected)
}

/// Build a validated install plan from raw install inputs.
pub fn plan_install(
    repository: &Path,
    profile: &str,
    integration_selection: Option<&str>,
) -> Result<InstallPlan, InstallError> {
    let profile = InstallProfile::from_str(profile)?;
    let integrations = parse_integration_selection(integration_selection)?;
    plan::build_install_plan(repository, profile, &integrations)
}

/// Validate install inputs, then apply the manifest-driven repository writes.
pub fn apply_install(
    repository: &Path,
    profile: &str,
    integration_selection: Option<&str>,
) -> Result<InstallReport, InstallError> {
    let plan = plan_install(repository, profile, integration_selection)?;
    writer::apply_install_plan(&plan)
}

/// Build a manifest-driven uninstall preview without mutating repository files.
pub fn plan_uninstall(repository: &Path) -> Result<UninstallPreview, InstallError> {
    uninstall_plan::build_uninstall_preview(repository)
}

/// Apply a previously planned uninstall preview to the repository.
pub fn apply_uninstall(
    repository: &Path,
    preview: &UninstallPreview,
) -> Result<UninstallApplyReport, InstallError> {
    writer::apply_uninstall_preview(repository, preview)
}
