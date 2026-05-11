//! Install profile/integration typing and catalog plumbing.

use std::collections::BTreeSet;
use std::path::Path;

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

// Re-export canonical install contract types from the shared contract crate.
// The enums and InstallSelection live in tanren_contract::install so both
// tanren-cli-app and tanren-testkit share a single definition site.
pub(crate) use tanren_contract::install::{InstallIntegration, InstallProfile, InstallSelection};

/// Build a validated install plan from raw install inputs.
pub(crate) fn plan_install(
    repository: &Path,
    profile: InstallProfile,
    integrations: &BTreeSet<InstallIntegration>,
) -> Result<InstallPlan, InstallError> {
    plan::build_install_plan(repository, profile, integrations)
}

/// Validate install inputs, then apply the manifest-driven repository writes.
pub(crate) fn apply_install(
    repository: &Path,
    profile: InstallProfile,
    integrations: &BTreeSet<InstallIntegration>,
) -> Result<InstallReport, InstallError> {
    let plan = plan_install(repository, profile, integrations)?;
    writer::apply_install_plan(&plan)
}

/// Validate install inputs, then analyze repository drift without mutations.
pub(crate) fn check_install_drift(
    repository: &Path,
    profile: InstallProfile,
    integrations: &BTreeSet<InstallIntegration>,
) -> Result<InstallDriftReport, InstallDriftError> {
    drift::check_install_drift(repository, profile, integrations)
}
