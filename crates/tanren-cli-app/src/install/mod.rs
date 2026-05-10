//! Install profile/integration typing and catalog plumbing.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

mod catalog;
mod cli;
#[cfg(feature = "test-hooks")]
pub(crate) mod contract;
mod drift;
mod error;
mod manifest;
mod path_guard;
mod plan;
mod writer;
mod writer_tx;

pub(crate) use cli::{DriftCommand, InstallCommand};
pub(crate) use drift::InstallDriftReport;
pub(crate) use error::{
    InstallCommandError, InstallDriftCommandError, InstallDriftError, InstallError,
};
#[cfg(feature = "test-hooks")]
pub(crate) use manifest::RepoRelativePath;
pub(crate) use plan::InstallPlan;
pub(crate) use writer::InstallReport;

/// Calculate a hex SHA-256 digest for test fixture bytes.
#[must_use]
#[cfg(feature = "test-hooks")]
pub(crate) fn sha256_hex(bytes: &[u8]) -> manifest::Sha256Hex {
    manifest::sha256_hex(bytes)
}

/// Supported Tanren standards profiles for local repository bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum InstallProfile {
    /// Install the Rust + Cargo standards profile.
    RustCargo,
}

impl InstallProfile {
    /// Canonical profile identifier.
    #[must_use]
    #[cfg(feature = "test-hooks")]
    pub(crate) const fn as_str(self) -> &'static str {
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
pub(crate) enum InstallIntegration {
    Claude,
    Codex,
    OpenCode,
}

impl InstallIntegration {
    /// Canonical integration identifier.
    #[must_use]
    #[cfg(feature = "test-hooks")]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "open-code",
        }
    }

    /// Return all supported integrations.
    #[must_use]
    pub(crate) fn all() -> BTreeSet<Self> {
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
pub(crate) fn parse_integration_selection(
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
pub(crate) fn plan_install(
    repository: &Path,
    profile: &str,
    integration_selection: Option<&str>,
) -> Result<InstallPlan, InstallError> {
    let profile = InstallProfile::from_str(profile)?;
    let integrations = parse_integration_selection(integration_selection)?;
    plan::build_install_plan(repository, profile, &integrations)
}

/// Validate install inputs, then apply the manifest-driven repository writes.
pub(crate) fn apply_install(
    repository: &Path,
    profile: &str,
    integration_selection: Option<&str>,
) -> Result<InstallReport, InstallError> {
    let plan = plan_install(repository, profile, integration_selection)?;
    writer::apply_install_plan(&plan)
}

/// Validate install inputs, then analyze repository drift without mutations.
pub(crate) fn check_install_drift(
    repository: &Path,
    profile: &str,
    integration_selection: Option<&str>,
) -> Result<InstallDriftReport, InstallDriftError> {
    drift::check_install_drift(repository, profile, integration_selection)
}
