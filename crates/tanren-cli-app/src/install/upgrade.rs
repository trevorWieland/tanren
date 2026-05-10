//! Upgrade planning and apply wiring for `tanren-cli upgrade`.

use std::path::{Path, PathBuf};

mod cli;
mod report;

use crate::install::error::InstallError;
use crate::install::manifest::{INSTALL_MANIFEST_REPO_PATH, InstallManifest, RepoRelativePath};
use crate::install::path_guard::resolve_repo_path;
use crate::install::{InstallPlan, InstallReport, plan_install};

pub use cli::UpgradeCommand;
use report::UpgradePreviewReport;

/// Typed outcome of an upgrade preview.
#[derive(Debug, Clone, PartialEq, Eq)]
enum UpgradePreview {
    /// Repository has no Tanren install manifest, so there is nothing to upgrade.
    NoInstallManifest,
    /// Repository has an install manifest and an apply plan was generated.
    Planned {
        plan: Box<InstallPlan>,
        report: UpgradePreviewReport,
    },
}

impl UpgradePreview {
    /// Build the preview report emitted to stdout.
    #[must_use]
    fn report(&self) -> UpgradePreviewReport {
        match self {
            Self::NoInstallManifest => UpgradePreviewReport::no_install_manifest(),
            Self::Planned { report, .. } => report.clone(),
        }
    }

    /// Whether this preview can be applied.
    #[must_use]
    const fn can_apply(&self) -> bool {
        matches!(self, Self::Planned { .. })
    }
}

/// Build an upgrade preview from the existing repository install manifest.
fn preview_upgrade(repository: &Path) -> Result<UpgradePreview, InstallError> {
    let repository_root = validate_repository_root(repository)?;
    let manifest_path = RepoRelativePath::parse(INSTALL_MANIFEST_REPO_PATH)?;
    let manifest_absolute_path = resolve_repo_path(&repository_root, &manifest_path)?;

    if !manifest_absolute_path.exists() {
        return Ok(UpgradePreview::NoInstallManifest);
    }

    let manifest = load_install_manifest(&manifest_path, &manifest_absolute_path)?;
    if manifest.integrations.is_empty() {
        return Err(InstallError::InvalidInstallManifest {
            path: manifest_path.as_str().to_owned(),
            message: "manifest integrations list cannot be empty".to_owned(),
        });
    }

    let integration_selection = manifest
        .integrations
        .iter()
        .map(|integration| integration.as_str())
        .collect::<Vec<_>>()
        .join(",");

    let plan = plan_install(
        &repository_root,
        manifest.profile.as_str(),
        Some(integration_selection.as_str()),
    )?;

    let report = UpgradePreviewReport::from_plan(&plan);
    Ok(UpgradePreview::Planned {
        plan: Box::new(plan),
        report,
    })
}

/// Apply a previously generated upgrade preview through the install writer.
fn apply_upgrade(preview: &UpgradePreview) -> Result<Option<InstallReport>, InstallError> {
    match preview {
        UpgradePreview::NoInstallManifest => Ok(None),
        UpgradePreview::Planned { plan, .. } => {
            let report = super::apply_validated_plan(plan.as_ref())?;
            Ok(Some(report))
        }
    }
}

fn load_install_manifest(
    manifest_path: &RepoRelativePath,
    manifest_absolute_path: &Path,
) -> Result<InstallManifest, InstallError> {
    let raw_manifest = std::fs::read_to_string(manifest_absolute_path).map_err(|err| {
        InstallError::ReadFailure {
            path: manifest_path.as_str().to_owned(),
            message: err.to_string(),
        }
    })?;

    toml::from_str(&raw_manifest).map_err(|err| InstallError::InvalidInstallManifest {
        path: manifest_path.as_str().to_owned(),
        message: err.to_string(),
    })
}

fn validate_repository_root(repository: &Path) -> Result<PathBuf, InstallError> {
    let canonical =
        repository
            .canonicalize()
            .map_err(|err| InstallError::InvalidRepositoryPath {
                path: format!("{} ({err})", display_repository_argument(repository)),
            })?;

    if !canonical.is_dir() {
        return Err(InstallError::RepositoryPathNotDirectory {
            path: display_repository_argument(repository),
        });
    }

    Ok(canonical)
}

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}
