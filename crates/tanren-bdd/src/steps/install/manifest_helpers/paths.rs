use std::path::PathBuf;

use tanren_testkit::InstallProofRepoRelativePath;

use crate::steps::install::InstallStepError;

/// BDD-scoped wrapper around the canonical
/// [`InstallProofRepoRelativePath`] from the install subsystem.
///
/// All path validation is delegated to the single canonical
/// `RepoRelativePath::parse` implementation; no validation logic
/// is duplicated in the BDD layer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RepositoryRelativePath(InstallProofRepoRelativePath);

impl RepositoryRelativePath {
    pub(crate) fn parse(raw: String) -> Result<Self, InstallStepError> {
        let inner = InstallProofRepoRelativePath::parse(raw.as_str())
            .map_err(|_| InstallStepError::RepositoryRelativePathRejected { path: raw })?;
        Ok(Self(inner))
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(super) fn as_install_path(&self) -> InstallProofRepoRelativePath {
        self.0.clone()
    }
}

pub(crate) fn validate_relative_path(path: &str) -> Result<(), InstallStepError> {
    InstallProofRepoRelativePath::parse(path).map_err(|_| {
        InstallStepError::RepositoryRelativePathRejected {
            path: path.to_owned(),
        }
    })?;
    Ok(())
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
