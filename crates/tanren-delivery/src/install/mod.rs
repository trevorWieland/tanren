//! Install profile/integration typing, catalog, planning, and manifest management.

use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;

mod apply_lock;
mod catalog;
#[cfg(feature = "test-hooks")]
pub mod contract;
mod error;
mod manifest;
mod manifest_entry_contract;
mod manifest_migration;
mod path_guard;
mod plan;
pub(crate) mod upgrade;
mod writer;
mod writer_tx;
mod writer_tx_support;

pub use error::InstallError;
pub use manifest::{ManifestVersion, RepoRelativePath};
pub use plan::InstallPlan;
pub use upgrade::{
    ApplyReportSummary, UpgradeApplyBlockedReason, UpgradeApplyConfirmation, UpgradeApplyOutcome,
    UpgradeCompatibilityConcern, UpgradePreview, UpgradePreviewReport, UpgradeWitnessRun,
    apply_upgrade, preview_upgrade, run_upgrade_witness,
};
pub use upgrade::{encode_field, format_encoded_path_list};
pub use writer::InstallReport;

/// Calculate a hex SHA-256 digest for test fixture bytes.
#[must_use]
#[cfg(feature = "test-hooks")]
pub fn sha256_hex(bytes: &[u8]) -> manifest::Sha256Hex {
    manifest::sha256_hex(bytes)
}

/// Calculate a hex SHA-256 digest for a file using streaming reads.
#[cfg(feature = "test-hooks")]
pub fn sha256_hex_file(path: &Path) -> Result<manifest::Sha256Hex, std::io::Error> {
    manifest::sha256_hex_file(path)
}

/// Install manifest with typed TOML parsing.
#[cfg(feature = "test-hooks")]
pub use manifest::InstallManifest;

/// Re-export manifest entry for test-hooks.
#[cfg(feature = "test-hooks")]
pub use manifest::ManifestEntry;

/// Cached catalog destination paths for fixture snapshot discovery.
#[cfg(feature = "test-hooks")]
pub fn cached_catalog_destination_paths(
    profile: InstallProfile,
) -> impl Iterator<Item = &'static str> {
    catalog::cached_catalog_destination_paths(profile)
        .iter()
        .map(|asset| asset.destination_path.as_str())
}

/// Supported Tanren standards profiles for local repository bootstrap.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
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

/// Parse integration selection from comma-separated values.
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
    apply_validated_plan(&plan)
}

pub(crate) fn apply_validated_plan(plan: &InstallPlan) -> Result<InstallReport, InstallError> {
    writer::apply_install_plan(plan)
}
