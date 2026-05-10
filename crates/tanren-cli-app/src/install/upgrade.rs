//! Manifest-aware upgrade planning and CLI for installed Tanren assets.

mod cli;
mod plan;
mod report;

use crate::install::error::InstallError;
use crate::install::plan::InstallPlan;
use crate::install::writer::{InstallReport, apply_install_plan};

pub use cli::UpgradeCommand;
pub use plan::{
    MigrationConcern, NothingToUpgrade, UpgradePlan, UpgradePlanOutcome, UpgradePlannedRemoval,
    UpgradePlannedWrite, UpgradePlannedWriteKind, plan_upgrade,
};
pub use report::UpgradePreviewReport;

/// Apply an upgrade plan through the transactional install writer path.
pub(crate) fn apply_upgrade(plan: &UpgradePlan) -> Result<InstallReport, InstallError> {
    let install_plan = InstallPlan::from_upgrade_plan(plan);
    apply_install_plan(&install_plan)
}
