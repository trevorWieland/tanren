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
