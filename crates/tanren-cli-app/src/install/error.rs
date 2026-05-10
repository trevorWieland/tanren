//! CLI-specific install and upgrade error wrappers.

use tanren_delivery::install::InstallError;
use thiserror::Error;

/// Typed `tanren-cli install` command failures at the CLI-library boundary.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum InstallCommandError {
    /// Input or selection validation failed before writes occurred.
    #[error("error: validation_failed — {source}")]
    ValidationFailed {
        #[source]
        source: InstallError,
    },
    /// Install planning or application failed.
    #[error("error: install_failed — {source}")]
    InstallFailed {
        #[source]
        source: InstallError,
    },
    /// Emitting success output to stdout failed.
    #[error("error: install_failed — write install report to stdout: {source}")]
    StdoutWriteFailure {
        #[source]
        source: std::io::Error,
    },
}

impl From<InstallError> for InstallCommandError {
    fn from(source: InstallError) -> Self {
        match source {
            InstallError::UnsupportedProfile { .. }
            | InstallError::UnsupportedIntegration { .. }
            | InstallError::EmptyIntegrationSelection
            | InstallError::InvalidRepositoryPath { .. }
            | InstallError::InvalidInstallManifest { .. }
            | InstallError::UnsafeRepositoryPath { .. }
            | InstallError::RepositoryPathNotDirectory { .. } => Self::ValidationFailed { source },
            _ => Self::InstallFailed { source },
        }
    }
}

/// Typed `tanren-cli upgrade` command failures at the CLI-library boundary.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum UpgradeCommandError {
    /// Input or manifest validation failed before writes occurred.
    #[error("error: validation_failed — {source}")]
    ValidationFailed {
        #[source]
        source: InstallError,
    },
    /// Upgrade planning or application failed.
    #[error("error: upgrade_failed — {source}")]
    UpgradeFailed {
        #[source]
        source: InstallError,
    },
    /// Emitting upgrade output to stdout failed.
    #[error("error: upgrade_failed — write upgrade report to stdout: {source}")]
    StdoutWriteFailure {
        #[source]
        source: std::io::Error,
    },
}

impl From<InstallError> for UpgradeCommandError {
    fn from(source: InstallError) -> Self {
        match source {
            InstallError::UnsupportedProfile { .. }
            | InstallError::UnsupportedIntegration { .. }
            | InstallError::EmptyIntegrationSelection
            | InstallError::InvalidRepositoryPath { .. }
            | InstallError::InvalidInstallManifest { .. }
            | InstallError::UnsafeRepositoryPath { .. }
            | InstallError::RepositoryPathNotDirectory { .. } => Self::ValidationFailed { source },
            _ => Self::UpgradeFailed { source },
        }
    }
}
