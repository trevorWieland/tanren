//! Test-only facade exports for downstream BDD and harness crates.

/// Install proof helpers exposed only when the `test-hooks` feature is on.
pub mod install {
    /// Parse failure for test-hook install relative paths.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RepoRelativePathParseError;

    /// Public wrapper over the internal install repo-relative path type.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct RepoRelativePath(crate::install::RepoRelativePath);

    impl RepoRelativePath {
        /// Validate and construct a repository-relative path.
        pub fn parse(path: &str) -> Result<Self, RepoRelativePathParseError> {
            crate::install::RepoRelativePath::parse(path)
                .map(Self)
                .map_err(|_| RepoRelativePathParseError)
        }

        /// Borrow the validated path string.
        #[must_use]
        pub fn as_str(&self) -> &str {
            self.0.as_str()
        }

        pub(super) fn as_inner(&self) -> &crate::install::RepoRelativePath {
            &self.0
        }
    }

    /// Calculate a hex SHA-256 digest for fixture bytes.
    #[must_use]
    #[cfg(feature = "test-hooks")]
    pub fn sha256_hex(bytes: &[u8]) -> String {
        crate::install::sha256_hex(bytes).to_string()
    }

    /// Install-proof contract helpers for BDD assertions and fixtures.
    pub mod contract {
        use super::RepoRelativePath;
        use std::path::Path;
        use thiserror::Error;

        /// Test-hook error surface for install proof assertions.
        #[derive(Debug, Error, Clone, PartialEq, Eq)]
        #[error("{message}")]
        pub struct InstallProofError {
            message: String,
        }

        impl From<crate::install::contract::InstallProofError> for InstallProofError {
            fn from(source: crate::install::contract::InstallProofError) -> Self {
                Self {
                    message: source.to_string(),
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
                relative_path.as_inner(),
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
            crate::install::contract::read_workspace_catalog_file(relative_path.as_inner())
                .map_err(InstallProofError::from)
        }
    }
}
