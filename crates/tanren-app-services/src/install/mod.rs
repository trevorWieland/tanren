//! Install command/result types and error surface.
//!
//! This module owns the app-service layer types for the Tanren bootstrap
//! install flow: the command input, the result output, the outcome enum, and
//! the domain errors. Wire shapes (manifest, profile name, content hash, etc.)
//! live in `tanren_contract::install`; this crate adds the orchestration-level
//! types that wrap or reference those contract shapes.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tanren_contract::{InstallContractError, InstallManifest, IntegrationName, ProfileName};
use thiserror::Error;

/// Input to the install operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallCommand {
    /// Standards profile to install.
    pub profile: ProfileName,
    /// Agent integrations to install. If empty, only standards are installed.
    pub integrations: Vec<IntegrationName>,
    /// Absolute path to the target repository root.
    pub target_path: PathBuf,
}

/// Output of a successful install operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    /// The manifest that was written to the repository.
    pub manifest: InstallManifest,
    /// Number of files written (created or replaced).
    pub files_written: usize,
    /// Number of stale generated files removed.
    pub files_removed: usize,
}

/// Discriminator for the install outcome.
#[derive(Debug)]
pub enum InstallOutcome {
    /// Install completed successfully.
    Success(InstallResult),
    /// Install was rejected before any files were written.
    Failure(InstallError),
}

/// Domain errors raised by the install operation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum InstallError {
    /// Contract-level validation failed (unknown profile or integration).
    #[error(transparent)]
    Contract(#[from] InstallContractError),
    /// The target path is not a valid directory.
    #[error("target path is not a valid directory: {0}")]
    InvalidTargetPath(String),
    /// A filesystem I/O error occurred during install.
    #[error("I/O error: {0}")]
    Io(String),
}
