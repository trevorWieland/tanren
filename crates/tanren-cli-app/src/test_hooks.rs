//! Test-only facade exports for downstream BDD and harness crates.

/// Install proof helpers exposed only when the `test-hooks` feature is on.
pub mod install {
    // Re-export the canonical RepoRelativePath from the shared contract crate
    // so there is exactly one struct definition in the workspace.
    pub use crate::install::RepoRelativePath;

    /// Calculate a hex SHA-256 digest for fixture bytes.
    #[must_use]
    #[cfg(feature = "test-hooks")]
    pub fn sha256_hex(bytes: &[u8]) -> String {
        crate::install::sha256_hex(bytes).to_string()
    }

    /// Install-proof contract helpers for BDD assertions and fixtures.
    pub mod contract {
        use super::RepoRelativePath;
        use std::path::{Path, PathBuf};
        use thiserror::Error;

        /// Structured install selection failures surfaced through test hooks.
        #[derive(Debug, Error, Clone, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum InstallSelectionError {
            #[error("unsupported install profile '{name}'")]
            UnsupportedProfile { name: String },
            #[error("unsupported install integration '{name}'")]
            UnsupportedIntegration { name: String },
            #[error("integration selection is empty")]
            EmptyIntegrationSelection,
            #[error(
                "catalog path must be repo-relative and cannot contain parent traversal: '{path}'"
            )]
            InvalidRepoRelativePath { path: String },
            #[error("repository path is invalid or inaccessible: '{path}'")]
            InvalidRepositoryPath { path: String },
            #[error("repository path does not exist or is not a directory: '{path}'")]
            RepositoryPathNotDirectory { path: String },
            #[error("repository path '{path}' is unsafe for install operations: {message}")]
            UnsafeRepositoryPath { path: String, message: String },
            #[error("install manifest at '{path}' is invalid: {message}")]
            InvalidInstallManifest { path: String, message: String },
            #[error("failed reading '{path}': {message}")]
            ReadFailure { path: String, message: String },
            #[error("failed creating directory '{path}': {message}")]
            CreateDirectoryFailure { path: String, message: String },
            #[error("failed writing '{path}': {message}")]
            WriteFailure { path: String, message: String },
            #[error("failed removing '{path}': {message}")]
            RemoveFailure { path: String, message: String },
        }

        impl From<crate::install::InstallError> for InstallSelectionError {
            fn from(source: crate::install::InstallError) -> Self {
                match source {
                    crate::install::InstallError::UnsupportedProfile { name } => {
                        Self::UnsupportedProfile { name }
                    }
                    crate::install::InstallError::UnsupportedIntegration { name } => {
                        Self::UnsupportedIntegration { name }
                    }
                    crate::install::InstallError::EmptyIntegrationSelection => {
                        Self::EmptyIntegrationSelection
                    }
                    crate::install::InstallError::InvalidRepoRelativePath { path } => {
                        Self::InvalidRepoRelativePath { path }
                    }
                    crate::install::InstallError::InvalidRepositoryPath { path } => {
                        Self::InvalidRepositoryPath { path }
                    }
                    crate::install::InstallError::RepositoryPathNotDirectory { path } => {
                        Self::RepositoryPathNotDirectory { path }
                    }
                    crate::install::InstallError::UnsafeRepositoryPath { path, message } => {
                        Self::UnsafeRepositoryPath { path, message }
                    }
                    crate::install::InstallError::InvalidInstallManifest { path, message } => {
                        Self::InvalidInstallManifest { path, message }
                    }
                    crate::install::InstallError::ReadFailure { path, message } => {
                        Self::ReadFailure { path, message }
                    }
                    crate::install::InstallError::CreateDirectoryFailure { path, message } => {
                        Self::CreateDirectoryFailure { path, message }
                    }
                    crate::install::InstallError::WriteFailure { path, message } => {
                        Self::WriteFailure { path, message }
                    }
                    crate::install::InstallError::RemoveFailure { path, message } => {
                        Self::RemoveFailure { path, message }
                    }
                    other => Self::UnsupportedProfile {
                        name: other.to_string(),
                    },
                }
            }
        }

        /// Install-proof failures surfaced through the test-hook contract facade.
        #[derive(Debug, Error)]
        #[non_exhaustive]
        pub enum InstallProofError {
            #[error("invalid integration assertion selection '{selection}': {source}")]
            InvalidIntegrationSelection {
                selection: String,
                source: crate::install::InstallError,
            },
            #[error("failed to canonicalize workspace root while {action}: {source}")]
            CanonicalizeWorkspaceRoot {
                action: &'static str,
                source: std::io::Error,
            },
            #[error("failed to read file '{path}' while {action}: {source}")]
            ReadFile {
                path: PathBuf,
                action: &'static str,
                source: std::io::Error,
            },
            #[error("failed to write file '{path}' while {action}: {source}")]
            WriteFile {
                path: PathBuf,
                action: &'static str,
                source: std::io::Error,
            },
            #[error("failed to read directory '{path}' while {action}: {source}")]
            ReadDirectory {
                path: PathBuf,
                action: &'static str,
                source: std::io::Error,
            },
            #[error("failed to inspect directory entry under '{path}' while {action}: {source}")]
            ReadDirectoryEntry {
                path: PathBuf,
                action: &'static str,
                source: std::io::Error,
            },
            #[error("failed to inspect file type for '{path}' while {action}: {source}")]
            InspectFileType {
                path: PathBuf,
                action: &'static str,
                source: std::io::Error,
            },
            #[error("failed to parse install manifest '{manifest_path}' as TOML: {source}")]
            InstallManifestTomlParse {
                manifest_path: PathBuf,
                source: toml::de::Error,
            },
            #[error("expected repository file to exist: {path}")]
            ExpectedFileToExist { path: PathBuf },
            #[error("expected repository path to be absent: {path}")]
            ExpectedFileToBeAbsent { path: PathBuf },
            #[error("expected fixture path to be absent before manifest injection: {path}")]
            StaleManifestPathAlreadyPresent { path: String },
            #[error(
                "install manifest '{manifest_path}' violated proof contract: {expected}\nmanifest:\n{manifest}"
            )]
            ManifestContractViolation {
                expected: String,
                manifest_path: PathBuf,
                manifest: String,
            },
        }

        impl From<crate::install::contract::InstallProofError> for InstallProofError {
            fn from(source: crate::install::contract::InstallProofError) -> Self {
                match source {
                    crate::install::contract::InstallProofError::InvalidIntegrationSelection {
                        selection,
                        source,
                    } => Self::InvalidIntegrationSelection {
                        selection,
                        source,
                    },
                    crate::install::contract::InstallProofError::CanonicalizeWorkspaceRoot {
                        action,
                        source,
                    } => Self::CanonicalizeWorkspaceRoot {
                        action,
                        source,
                    },
                    crate::install::contract::InstallProofError::ReadFile {
                        path,
                        action,
                        source,
                    } => Self::ReadFile {
                        path,
                        action,
                        source,
                    },
                    crate::install::contract::InstallProofError::WriteFile {
                        path,
                        action,
                        source,
                    } => Self::WriteFile {
                        path,
                        action,
                        source,
                    },
                    crate::install::contract::InstallProofError::ReadDirectory {
                        path,
                        action,
                        source,
                    } => Self::ReadDirectory {
                        path,
                        action,
                        source,
                    },
                    crate::install::contract::InstallProofError::ReadDirectoryEntry {
                        path,
                        action,
                        source,
                    } => Self::ReadDirectoryEntry {
                        path,
                        action,
                        source,
                    },
                    crate::install::contract::InstallProofError::InspectFileType {
                        path,
                        action,
                        source,
                    } => Self::InspectFileType {
                        path,
                        action,
                        source,
                    },
                    crate::install::contract::InstallProofError::InstallManifestTomlParse {
                        manifest_path,
                        source,
                    } => Self::InstallManifestTomlParse {
                        manifest_path,
                        source,
                    },
                    crate::install::contract::InstallProofError::ExpectedFileToExist { path } => {
                        Self::ExpectedFileToExist { path }
                    }
                    crate::install::contract::InstallProofError::ExpectedFileToBeAbsent {
                        path,
                    } => Self::ExpectedFileToBeAbsent { path },
                    crate::install::contract::InstallProofError::StaleManifestPathAlreadyPresent {
                        path,
                    } => Self::StaleManifestPathAlreadyPresent { path },
                    crate::install::contract::InstallProofError::ManifestContractViolation {
                        expected,
                        manifest_path,
                        manifest,
                    } => Self::ManifestContractViolation {
                        expected,
                        manifest_path,
                        manifest,
                    },
                }
            }
        }

        /// Assert the default rust-cargo install writes command and standards assets.
        pub fn assert_rust_cargo_default_assets_installed(
            repository_root: &Path,
        ) -> Result<(), InstallProofError> {
            crate::install::contract::assert_rust_cargo_default_assets_installed(repository_root)
                .map_err(InstallProofError::from)
        }

        /// Assert rust-cargo standards profile assets are installed.
        pub fn assert_rust_cargo_standards_installed(
            repository_root: &Path,
        ) -> Result<(), InstallProofError> {
            crate::install::contract::assert_rust_cargo_standards_installed(repository_root)
                .map_err(InstallProofError::from)
        }

        /// Assert only selected integration command assets are installed.
        pub fn assert_selected_integration_command_assets(
            repository_root: &Path,
            selected_integrations: &str,
        ) -> Result<(), InstallProofError> {
            crate::install::contract::assert_selected_integration_command_assets(
                repository_root,
                selected_integrations,
            )
            .map_err(InstallProofError::from)
        }

        /// Assert install manifest defaults for rust-cargo profile installs.
        pub fn assert_manifest_rust_cargo_defaults(
            repository_root: &Path,
        ) -> Result<(), InstallProofError> {
            crate::install::contract::assert_manifest_rust_cargo_defaults(repository_root)
                .map_err(InstallProofError::from)
        }

        /// Append a stale generated-manifest row for mutation-flow fixtures.
        #[cfg(feature = "test-hooks")]
        pub fn append_stale_generated_manifest_entry(
            manifest: &mut String,
            relative_path: &RepoRelativePath,
            content_hash: &str,
        ) {
            crate::install::contract::append_stale_generated_manifest_entry(
                manifest,
                relative_path,
                content_hash,
            );
        }

        /// Inject a raw stale generated-manifest row (used by traversal tamper witnesses).
        pub fn tamper_manifest_with_raw_generated_entry(
            repository_root: &Path,
            raw_path: &str,
        ) -> Result<(), InstallProofError> {
            crate::install::contract::tamper_manifest_with_raw_generated_entry(
                repository_root,
                raw_path,
            )
            .map_err(InstallProofError::from)
        }

        /// Read a workspace catalog file for fixture seeding.
        #[cfg(feature = "test-hooks")]
        pub fn read_workspace_catalog_file(
            relative_path: &RepoRelativePath,
        ) -> Result<String, InstallProofError> {
            crate::install::contract::read_workspace_catalog_file(relative_path)
                .map_err(InstallProofError::from)
        }
    }
}
