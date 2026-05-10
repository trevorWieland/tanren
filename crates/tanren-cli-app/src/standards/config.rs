//! Standards command repository/config resolution.

use std::fs;
use std::path::{Path, PathBuf};

use tanren_configuration_secrets::{MethodologyProfile, ProjectMethodologyConfig, StandardsRoot};

use crate::install::resolve_repo_relative_path;

use super::error::{StandardsCommandError, StandardsError};

const PROJECT_METHODOLOGY_CONFIG_REPO_PATH: &str = ".tanren/project-methodology.toml";

/// Resolved config inputs for a standards inspection run.
#[derive(Debug, Clone)]
pub(super) struct StandardsInspectionTargets {
    repository_root: PathBuf,
    profile: MethodologyProfile,
    standards_root: StandardsRoot,
    standards_root_path: PathBuf,
}

impl StandardsInspectionTargets {
    pub(super) fn repository_root(&self) -> &Path {
        self.repository_root.as_path()
    }

    pub(super) fn profile(&self) -> MethodologyProfile {
        self.profile
    }

    pub(super) fn standards_root(&self) -> &StandardsRoot {
        &self.standards_root
    }

    pub(super) fn standards_root_path(&self) -> &Path {
        self.standards_root_path.as_path()
    }
}

pub(super) fn resolve_inspection_targets(
    repository: &Path,
) -> Result<StandardsInspectionTargets, StandardsCommandError> {
    let repository_argument = display_repository_argument(repository);
    let repository_root = repository.canonicalize().map_err(|_| {
        StandardsCommandError::validation_failed(StandardsError::InvalidRepositoryPath {
            path: repository_argument.clone(),
        })
    })?;

    if !repository_root.is_dir() {
        return Err(StandardsCommandError::validation_failed(
            StandardsError::RepositoryPathNotDirectory {
                path: repository_argument,
            },
        ));
    }

    let config = load_project_methodology_config(&repository_root)?;
    let standards_root_relative = config.standards_root.as_str().to_owned();
    let standards_root_path = resolve_repo_relative_path(
        &repository_root,
        config.standards_root.as_str(),
    )
    .map_err(|source| {
        StandardsCommandError::validation_failed(StandardsError::InvalidConfiguredStandardsRoot {
            path: standards_root_relative.clone(),
            source,
        })
    })?;

    if !standards_root_path.is_dir() {
        return Err(StandardsCommandError::standards_missing(
            StandardsError::StandardsRootMissing {
                path: standards_root_relative,
            },
        ));
    }

    Ok(StandardsInspectionTargets {
        repository_root,
        profile: config.profile,
        standards_root: config.standards_root,
        standards_root_path,
    })
}

fn load_project_methodology_config(
    repository_root: &Path,
) -> Result<ProjectMethodologyConfig, StandardsCommandError> {
    let config_path = repository_root.join(PROJECT_METHODOLOGY_CONFIG_REPO_PATH);
    let config_text = fs::read_to_string(&config_path).map_err(|source| {
        StandardsCommandError::validation_failed(StandardsError::ReadFailure {
            path: PROJECT_METHODOLOGY_CONFIG_REPO_PATH.to_owned(),
            source,
        })
    })?;

    let config = ProjectMethodologyConfig::from_toml(&config_text).map_err(|source| {
        StandardsCommandError::validation_failed(StandardsError::ProjectMethodologyConfigParse {
            path: PROJECT_METHODOLOGY_CONFIG_REPO_PATH.to_owned(),
            source,
        })
    })?;

    config.schema_version.ensure_supported().map_err(|source| {
        StandardsCommandError::validation_failed(
            StandardsError::ProjectMethodologyConfigIncompatible {
                path: PROJECT_METHODOLOGY_CONFIG_REPO_PATH.to_owned(),
                source,
            },
        )
    })?;

    Ok(config)
}

pub(super) fn to_repo_relative_path(
    repository_root: &Path,
    path: &Path,
) -> Result<String, StandardsError> {
    let relative = path.strip_prefix(repository_root).map_err(|_| {
        StandardsError::NonRepositoryRelativePath {
            path: display_repository_argument(path),
        }
    })?;
    Ok(relative.display().to_string())
}

pub(super) fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}
