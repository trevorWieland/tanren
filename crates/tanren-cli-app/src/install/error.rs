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
}
