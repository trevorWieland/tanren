//! Installer domain errors.

use thiserror::Error;

/// Typed install-input and catalog validation errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InstallError {
    /// The requested standards profile is not supported.
    #[error("unsupported install profile '{name}'")]
    UnsupportedProfile { name: String },
    /// The requested integration is not supported.
    #[error("unsupported install integration '{name}'")]
    UnsupportedIntegration { name: String },
    /// Integration selection was provided but contained no integration names.
    #[error("integration selection is empty")]
    EmptyIntegrationSelection,
    /// The catalog path is not repository-relative.
    #[error("catalog path must be repo-relative and cannot contain parent traversal: '{path}'")]
    InvalidRepoRelativePath { path: String },
    /// Repository root path is invalid for install planning.
    #[error("repository path is invalid or inaccessible: '{path}'")]
    InvalidRepositoryPath { path: String },
    /// Repository root must already exist and be a directory.
    #[error("repository path does not exist or is not a directory: '{path}'")]
    RepositoryPathNotDirectory { path: String },
    /// Resolved path exits the repository boundary or traverses symlink components.
    #[error("repository path '{path}' is unsafe for install operations: {message}")]
    UnsafeRepositoryPath { path: String, message: String },
    /// Reading or parsing install manifest failed.
    #[error("install manifest at '{path}' is invalid: {message}")]
    InvalidInstallManifest { path: String, message: String },
    /// An expected repository read failed.
    #[error("failed reading '{path}': {message}")]
    ReadFailure { path: String, message: String },
    /// Creating a parent directory for install output failed.
    #[error("failed creating directory '{path}': {message}")]
    CreateDirectoryFailure { path: String, message: String },
    /// Writing repository output failed.
    #[error("failed writing '{path}': {message}")]
    WriteFailure { path: String, message: String },
    /// Removing stale generated output failed.
    #[error("failed removing '{path}': {message}")]
    RemoveFailure { path: String, message: String },
}

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
    /// Uninstall planning or application failed.
    #[error("error: uninstall_failed — {source}")]
    UninstallFailed {
        #[source]
        source: InstallError,
    },
    /// Emitting success output to stdout failed.
    #[error("error: command_failed — write command report to stdout: {source}")]
    StdoutWriteFailure {
        #[source]
        source: std::io::Error,
    },
}

impl From<InstallError> for InstallCommandError {
    fn from(source: InstallError) -> Self {
        Self::from_install_error(source)
    }
}

impl InstallCommandError {
    fn from_install_error(source: InstallError) -> Self {
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

    pub(super) fn from_uninstall_error(source: InstallError) -> Self {
        match source {
            InstallError::InvalidRepositoryPath { .. }
            | InstallError::InvalidInstallManifest { .. }
            | InstallError::UnsafeRepositoryPath { .. }
            | InstallError::RepositoryPathNotDirectory { .. } => Self::ValidationFailed { source },
            _ => Self::UninstallFailed { source },
        }
    }
}
