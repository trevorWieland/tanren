//! Upgrade planning and apply wiring for Tanren repository assets.

use std::collections::BTreeSet;
use std::path::Path;

mod report;

use crate::install::error::InstallError;
use crate::install::plan::{build_install_plan_from_state, load_repository_install_state};
use crate::install::{InstallIntegration, InstallPlan, InstallReport};

pub use report::UpgradePreviewReport;

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
pub enum UpgradePreview {
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
    pub fn report(&self) -> &UpgradePreviewReport {
        match self {
            Self::NoInstallManifest { report } | Self::Planned { report, .. } => report,
        }
    }

    /// Whether this preview can be applied.
    #[must_use]
    pub const fn can_apply(&self) -> bool {
        matches!(self, Self::Planned { .. })
    }
}

/// Build an upgrade preview from the existing repository install manifest.
pub fn preview_upgrade(repository: &Path) -> Result<UpgradePreview, InstallError> {
    UpgradePlanner::preview(repository)
}

/// Reason an upgrade apply operation was blocked before mutating files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeApplyBlockedReason {
    /// Preview did not produce an applyable plan.
    PreviewNotApplicable,
}

impl UpgradeApplyBlockedReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreviewNotApplicable => "preview_not_applicable",
        }
    }
}

/// Typed outcome for applying an upgrade preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeApplyOutcome {
    /// No install manifest was present, so apply is a no-op.
    NoInstallManifestNoop,
    /// Upgrade plan applied successfully.
    Applied { report: InstallReport },
    /// Apply did not run because a precondition was not met.
    Blocked { reason: UpgradeApplyBlockedReason },
}

impl UpgradeApplyOutcome {
    const NO_MANIFEST_NOOP_LABEL: &str = "no_manifest_noop";
    const APPLIED_LABEL: &str = "applied";
    const BLOCKED_LABEL: &str = "blocked";

    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::NoInstallManifestNoop => Self::NO_MANIFEST_NOOP_LABEL,
            Self::Applied { .. } => Self::APPLIED_LABEL,
            Self::Blocked { .. } => Self::BLOCKED_LABEL,
        }
    }
}

/// Apply a previously generated upgrade preview through the install writer.
pub fn apply_upgrade(preview: &UpgradePreview) -> Result<UpgradeApplyOutcome, InstallError> {
    if let UpgradePreview::Planned { plan, .. } = preview {
        let report = super::apply_validated_plan(plan.as_ref())?;
        return Ok(UpgradeApplyOutcome::Applied { report });
    }
    if matches!(preview, UpgradePreview::NoInstallManifest { .. }) {
        return Ok(UpgradeApplyOutcome::NoInstallManifestNoop);
    }
    Ok(UpgradeApplyOutcome::Blocked {
        reason: UpgradeApplyBlockedReason::PreviewNotApplicable,
    })
}

/// Structured outcome for web/API witnesses that need the same observable
/// upgrade report without shelling out to the CLI binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeWitnessRun {
    /// Canonical stdout lines that mirror `tanren-cli upgrade` output.
    pub stdout_lines: Vec<String>,
}

impl UpgradeWitnessRun {
    /// Join `stdout_lines` into a newline-delimited payload.
    #[must_use]
    pub fn stdout(&self) -> String {
        self.stdout_lines.join("\n")
    }
}

/// Run `upgrade` in-process and render the same machine-readable witness lines
/// the CLI prints to stdout.
pub fn run_upgrade_witness(
    repository: &Path,
    confirm: bool,
) -> Result<UpgradeWitnessRun, InstallError> {
    let preview = preview_upgrade(repository)?;
    let mut lines = Vec::new();
    let render = preview.report().render();

    lines.push(format!(
        "status=preview command=upgrade repo={} changed={} destructive={} preserved={} concerns={}",
        display_repository_argument(repository),
        render.changed_count(),
        render.destructive_count(),
        render.preserved_count(),
        render.concern_count(),
    ));
    lines.push(format!(
        "preview changed=[{}] destructive=[{}] preserved=[{}] concerns=[{}]",
        render.changed_paths_csv(),
        render.destructive_paths_csv(),
        render.preserved_paths_csv(),
        render.concern_codes_csv(),
    ));

    if !confirm {
        lines.push(format!(
            "status=confirmation_required command=upgrade repo={} confirm=false applied=false writes=0 removals=0 preserved=0 can_apply={}",
            display_repository_argument(repository),
            preview.can_apply(),
        ));
        return Ok(UpgradeWitnessRun {
            stdout_lines: lines,
        });
    }

    let apply_outcome = apply_upgrade(&preview)?;

    match apply_outcome {
        UpgradeApplyOutcome::NoInstallManifestNoop => {
            lines.push(format!(
                "status=noop command=upgrade repo={} confirm=true applied=false outcome={} created=0 updated=0 removed=0 restored=0 preserved=0",
                display_repository_argument(repository),
                UpgradeApplyOutcome::NoInstallManifestNoop.label(),
            ));
        }
        UpgradeApplyOutcome::Applied { report } => {
            lines.push(format!(
                "status=ok command=upgrade repo={} confirm=true applied=true outcome={} created={} updated={} removed={} restored={} preserved={}",
                display_repository_argument(repository),
                UpgradeApplyOutcome::APPLIED_LABEL,
                report.created.len(),
                report.updated.len(),
                report.removed.len(),
                report.restored.len(),
                report.preserved.len(),
            ));
            lines.push(format!(
                "applied created=[{}] updated=[{}] removed=[{}] restored=[{}] preserved=[{}]",
                format_path_list(&report.created),
                format_path_list(&report.updated),
                format_path_list(&report.removed),
                format_path_list(&report.restored),
                format_path_list(&report.preserved),
            ));
        }
        UpgradeApplyOutcome::Blocked { reason } => {
            lines.push(format!(
                "status=blocked command=upgrade repo={} confirm=true applied=false outcome={} reason={} created=0 updated=0 removed=0 restored=0 preserved=0",
                display_repository_argument(repository),
                UpgradeApplyOutcome::Blocked { reason }.label(),
                reason.as_str(),
            ));
        }
    }

    Ok(UpgradeWitnessRun {
        stdout_lines: lines,
    })
}

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}

fn format_path_list(paths: &[crate::install::manifest::RepoRelativePath]) -> String {
    paths
        .iter()
        .map(crate::install::manifest::RepoRelativePath::as_str)
        .collect::<Vec<_>>()
        .join(",")
}
