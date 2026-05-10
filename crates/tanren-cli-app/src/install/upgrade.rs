//! Manifest-aware upgrade planning for installed Tanren assets.

mod plan;

pub use plan::{
    MigrationConcern, NothingToUpgrade, UpgradePlan, UpgradePlanOutcome, UpgradePlannedRemoval,
    UpgradePlannedWrite, UpgradePlannedWriteKind, plan_upgrade,
};
