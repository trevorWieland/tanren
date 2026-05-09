use std::path::{Component, Path, PathBuf};

use tanren_cli_app::install::manifest::RepoRelativePath;

use crate::steps::install::InstallStepError;

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

    pub(super) fn as_install_path(&self) -> Result<RepoRelativePath, InstallStepError> {
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
