//! Upgrade planning and apply wiring for `tanren-cli upgrade`.

use std::collections::BTreeSet;
use std::path::Path;

mod cli;
mod report;

use crate::install::error::InstallError;
use crate::install::plan::{build_install_plan_from_state, load_repository_install_state};
use crate::install::{InstallIntegration, InstallPlan, InstallReport};

pub use cli::UpgradeCommand;
use report::UpgradePreviewReport;

/// Upgrade planner boundary for manifest-driven upgrade previews.
#[derive(Debug, Clone, Copy, Default)]
struct UpgradePlanner;

impl UpgradePlanner {
    fn preview(repository: &Path) -> Result<UpgradePreview, InstallError> {
        let state = load_repository_install_state(repository)?;
        let Some(previous_manifest) = state.previous_manifest().cloned() else {
            return Ok(UpgradePreview::NoInstallManifest {
                report: UpgradePreviewReport::no_install_manifest(),
            });
        };
        if previous_manifest.integrations.is_empty() {
            return Err(InstallError::InvalidInstallManifest {
                path: state.manifest_path().as_str().to_owned(),
                message: "manifest integrations list cannot be empty".to_owned(),
            });
        }

        let integrations = previous_manifest
            .integrations
            .iter()
            .copied()
            .collect::<BTreeSet<InstallIntegration>>();
        let plan = build_install_plan_from_state(state, previous_manifest.profile, &integrations)?;
        let report = UpgradePreviewReport::from_plan(&plan);
        Ok(UpgradePreview::Planned {
            plan: Box::new(plan),
            report,
        })
    }
}

/// Typed outcome of an upgrade preview.
#[derive(Debug, Clone, PartialEq, Eq)]
enum UpgradePreview {
    /// Repository has no Tanren install manifest, so there is nothing to upgrade.
    NoInstallManifest { report: UpgradePreviewReport },
    /// Repository has an install manifest and an apply plan was generated.
    Planned {
        plan: Box<InstallPlan>,
        report: UpgradePreviewReport,
    },
}

impl UpgradePreview {
    /// Build the preview report emitted to stdout.
    #[must_use]
    fn report(&self) -> &UpgradePreviewReport {
        match self {
            Self::NoInstallManifest { report } | Self::Planned { report, .. } => report,
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
    UpgradePlanner::preview(repository)
}

/// Apply a previously generated upgrade preview through the install writer.
fn apply_upgrade(preview: &UpgradePreview) -> Result<Option<InstallReport>, InstallError> {
    match preview {
        UpgradePreview::NoInstallManifest { .. } => Ok(None),
        UpgradePreview::Planned { plan, .. } => {
            let report = super::apply_validated_plan(plan.as_ref())?;
            Ok(Some(report))
        }
    }
}
