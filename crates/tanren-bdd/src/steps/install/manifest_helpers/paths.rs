use std::path::{Component, Path, PathBuf};

use tanren_testkit::InstallProofRepoRelativePath;

use crate::steps::install::InstallStepError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RepositoryRelativePath(String);

impl RepositoryRelativePath {
    pub(crate) fn parse(raw: String) -> Result<Self, InstallStepError> {
        validate_relative_path(raw.as_str())?;
        let normalized: String = Path::new(&raw)
            .components()
            .filter_map(|component| match component {
                Component::Normal(segment) => Some(segment.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/");
        drop(raw);
        Ok(Self(normalized))
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(super) fn as_install_path(&self) -> Result<InstallProofRepoRelativePath, InstallStepError> {
        InstallProofRepoRelativePath::parse(self.as_str()).map_err(|_| {
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
    let has_only_normal_or_curdir = candidate
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
    if !has_only_normal_or_curdir {
        return Err(InstallStepError::TraversalRepositoryRelativePath {
            path: path.to_owned(),
        });
    }
    let has_normal = candidate
        .components()
        .any(|component| matches!(component, Component::Normal(_)));
    if !has_normal {
        return Err(InstallStepError::EmptyRepositoryRelativePath);
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
