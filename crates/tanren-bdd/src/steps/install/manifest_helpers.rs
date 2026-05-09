use std::path::{Component, Path, PathBuf};

use tanren_cli_app::install::contract;
use tanren_cli_app::install::manifest::{RepoRelativePath, sha256_hex};

use super::InstallStepError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RepositoryRelativePath(String);

impl RepositoryRelativePath {
    pub(crate) fn parse(raw: String) -> Result<Self, InstallStepError> {
        validate_relative_path(raw.as_str())?;
        Ok(Self(raw))
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }

    fn as_install_path(&self) -> Result<RepoRelativePath, InstallStepError> {
        RepoRelativePath::parse(self.as_str()).map_err(|_| {
            InstallStepError::InstallPathContractRejected {
                path: self.as_str().to_owned(),
            }
        })
    }
}

pub(crate) fn validate_relative_path(path: &str) -> Result<(), InstallStepError> {
    if path.is_empty() {
        return Err(InstallStepError::EmptyRepositoryRelativePath);
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err(InstallStepError::AbsoluteRepositoryRelativePath {
            path: path.to_owned(),
        });
    }
    let is_valid = candidate
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
    if !is_valid {
        return Err(InstallStepError::TraversalRepositoryRelativePath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn assert_rust_cargo_default_assets_installed(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    contract::assert_rust_cargo_default_assets_installed(repository_root)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn assert_rust_cargo_standards_installed(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    contract::assert_rust_cargo_standards_installed(repository_root)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn assert_selected_integration_command_assets(
    repository_root: &Path,
    selected_integrations: &str,
) -> Result<(), InstallStepError> {
    contract::assert_selected_integration_command_assets(repository_root, selected_integrations)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn assert_manifest_rust_cargo_defaults(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    contract::assert_manifest_rust_cargo_defaults(repository_root)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn append_stale_generated_manifest_entry(
    manifest: &mut String,
    relative_path: &RepositoryRelativePath,
    content_hash: &str,
) -> Result<(), InstallStepError> {
    contract::append_stale_generated_manifest_entry(
        manifest,
        &relative_path.as_install_path()?,
        content_hash,
    );
    Ok(())
}

pub(crate) fn tamper_manifest_with_raw_generated_entry(
    repository_root: &Path,
    raw_path: &str,
) -> Result<(), InstallStepError> {
    contract::tamper_manifest_with_raw_generated_entry(repository_root, raw_path)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn sha256_hex_string(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

pub(crate) fn read_workspace_catalog_file(relative_path: &str) -> Result<String, InstallStepError> {
    validate_relative_path(relative_path)?;
    let parsed = RepoRelativePath::parse(relative_path).map_err(|_| {
        InstallStepError::InstallPathContractRejected {
            path: relative_path.to_owned(),
        }
    })?;
    contract::read_workspace_catalog_file(&parsed)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn io_error(
    path: PathBuf,
    action: &'static str,
    source: std::io::Error,
) -> InstallStepError {
    InstallStepError::Io {
        path,
        action,
        source,
    }
}
