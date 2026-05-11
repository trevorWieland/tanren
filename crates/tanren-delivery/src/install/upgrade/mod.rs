//! Upgrade planning and apply wiring for Tanren repository assets.

use std::collections::BTreeSet;
use std::path::Path;

mod report;

use crate::install::error::InstallError;
use crate::install::manifest::InstallManifest;
use crate::install::manifest_migration::{
    ManifestLoadOutcome, ManifestMigrationOutcome, migrate_manifest, validate_migrated_manifest,
};
use crate::install::plan::{build_install_plan_from_state, load_repository_install_state};
use crate::install::{InstallIntegration, InstallPlan, InstallReport};

pub use report::{
    PreviewId, UpgradeCompatibilityConcern, UpgradePreviewReport, encode_field,
    format_encoded_path_list,
};

/// Upgrade planner boundary for manifest-driven upgrade previews.
#[derive(Debug, Clone, Copy, Default)]
struct UpgradePlanner;

/// Load the previous manifest with migration awareness for upgrade preview.
///
/// Unlike [`load_repository_install_state`], this returns
/// [`ManifestLoadOutcome::UnsupportedVersion`] instead of erroring,
/// so the preview can surface the compatibility concern.
fn preview_load_manifest(repository: &Path) -> Result<ManifestLoadOutcome, InstallError> {
    use crate::install::manifest::INSTALL_MANIFEST_REPO_PATH;
    use crate::install::path_guard::resolve_repo_path;
    use crate::install::plan::validate_repository_root;

    let repository_root = validate_repository_root(repository)?;
    let manifest_path =
        crate::install::manifest::RepoRelativePath::parse(INSTALL_MANIFEST_REPO_PATH)?;
    let manifest_absolute_path = resolve_repo_path(&repository_root, &manifest_path)?;
    if !manifest_absolute_path.exists() {
        return Ok(ManifestLoadOutcome::NoManifest);
    }
    let raw = std::fs::read_to_string(&manifest_absolute_path).map_err(|err| {
        InstallError::ReadFailure {
            path: manifest_path.as_str().to_owned(),
            message: err.to_string(),
        }
    })?;
    let manifest: InstallManifest =
        toml::from_str(&raw).map_err(|err| InstallError::InvalidInstallManifest {
            path: manifest_path.as_str().to_owned(),
            message: err.to_string(),
        })?;
    let migration_outcome = migrate_manifest(manifest);
    match migration_outcome {
        ManifestMigrationOutcome::CurrentVersion { manifest }
        | ManifestMigrationOutcome::Migrated { manifest, .. } => {
            let validated = validate_migrated_manifest(manifest, manifest_path.as_str())?;
            Ok(ManifestLoadOutcome::Validated(validated))
        }
        ManifestMigrationOutcome::UnsupportedVersion {
            detected,
            min_supported,
            current,
        } => Ok(ManifestLoadOutcome::UnsupportedVersion {
            detected,
            min_supported,
            current,
        }),
    }
}

impl UpgradePlanner {
    fn preview(repository: &Path) -> Result<UpgradePreview, InstallError> {
        let load_result = preview_load_manifest(repository)?;
        match load_result {
            ManifestLoadOutcome::NoManifest => Ok(UpgradePreview::NoInstallManifest {
                report: UpgradePreviewReport::no_install_manifest(),
            }),
            ManifestLoadOutcome::Validated(_) => {
                let state = load_repository_install_state(repository)?;
                let Some(previous_manifest) = state.previous_manifest().cloned() else {
                    return Ok(UpgradePreview::NoInstallManifest {
                        report: UpgradePreviewReport::no_install_manifest(),
                    });
                };
                let integrations = previous_manifest
                    .integrations
                    .iter()
                    .copied()
                    .collect::<BTreeSet<InstallIntegration>>();
                let plan =
                    build_install_plan_from_state(state, previous_manifest.profile, &integrations)?;
                let report = UpgradePreviewReport::from_plan(&plan);
                Ok(UpgradePreview::Planned {
                    plan: Box::new(plan),
                    report,
                })
            }
            ManifestLoadOutcome::UnsupportedVersion {
                detected,
                min_supported,
                current,
            } => Ok(UpgradePreview::UnsupportedManifestVersion {
                report: UpgradePreviewReport::unsupported_manifest_version(
                    detected,
                    min_supported,
                    current,
                ),
            }),
        }
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
    /// Manifest version is not supported for migration.
    UnsupportedManifestVersion { report: UpgradePreviewReport },
}

impl UpgradePreview {
    /// Build the preview report emitted to stdout.
    #[must_use]
    pub fn report(&self) -> &UpgradePreviewReport {
        match self {
            Self::NoInstallManifest { report }
            | Self::Planned { report, .. }
            | Self::UnsupportedManifestVersion { report } => report,
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

/// Explicit confirmation token required before upgrade apply proceeds.
///
/// The caller constructs [`UpgradeApplyConfirmation::confirmed`] only after
/// the user or automation layer has acknowledged the preview. This prevents
/// accidental applies without confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpgradeApplyConfirmation {
    confirmed: bool,
}

impl UpgradeApplyConfirmation {
    /// Create a confirmed apply token.
    #[must_use]
    pub const fn confirmed() -> Self {
        Self { confirmed: true }
    }

    /// Whether the token represents explicit confirmation.
    #[must_use]
    pub const fn is_confirmed(self) -> bool {
        self.confirmed
    }
}

/// Reason an upgrade apply operation was blocked before mutating files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeApplyBlockedReason {
    /// Preview did not produce an applyable plan.
    PreviewNotApplicable,
    /// Caller did not supply explicit confirmation.
    ConfirmationRequired,
}

impl UpgradeApplyBlockedReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreviewNotApplicable => "preview_not_applicable",
            Self::ConfirmationRequired => "confirmation_required",
        }
    }
}

/// Typed outcome for applying an upgrade preview.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum UpgradeApplyOutcome {
    /// No install manifest was present, so apply is a no-op.
    NoManifestNoop,
    /// Upgrade plan applied successfully.
    Applied { report: ApplyReportSummary },
    /// Apply did not run because a precondition was not met.
    Blocked { reason: UpgradeApplyBlockedReason },
}

/// Summary of an applied install report suitable for serialization.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ApplyReportSummary {
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    pub restored: Vec<String>,
    pub preserved: Vec<String>,
}

impl ApplyReportSummary {
    fn from_report(report: &InstallReport) -> Self {
        Self {
            created: report
                .created
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            updated: report
                .updated
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            removed: report
                .removed
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            restored: report
                .restored
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
            preserved: report
                .preserved
                .iter()
                .map(|p| p.as_str().to_owned())
                .collect(),
        }
    }
}

impl UpgradeApplyOutcome {
    const NO_MANIFEST_NOOP_LABEL: &str = "no_manifest_noop";
    const APPLIED_LABEL: &str = "applied";
    const BLOCKED_LABEL: &str = "blocked";

    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::NoManifestNoop => Self::NO_MANIFEST_NOOP_LABEL,
            Self::Applied { .. } => Self::APPLIED_LABEL,
            Self::Blocked { .. } => Self::BLOCKED_LABEL,
        }
    }
}

/// Apply a previously generated upgrade preview through the install writer.
///
/// Requires explicit [`UpgradeApplyConfirmation`] to proceed. Returns a typed
/// [`UpgradeApplyOutcome`] instead of a status integer.
pub fn apply_upgrade(
    preview: &UpgradePreview,
    confirmation: UpgradeApplyConfirmation,
) -> Result<UpgradeApplyOutcome, InstallError> {
    if !confirmation.is_confirmed() {
        return Ok(UpgradeApplyOutcome::Blocked {
            reason: UpgradeApplyBlockedReason::ConfirmationRequired,
        });
    }
    if let UpgradePreview::Planned { plan, .. } = preview {
        let report = super::apply_validated_plan(plan.as_ref())?;
        let summary = ApplyReportSummary::from_report(&report);
        return Ok(UpgradeApplyOutcome::Applied { report: summary });
    }
    if matches!(preview, UpgradePreview::NoInstallManifest { .. }) {
        return Ok(UpgradeApplyOutcome::NoManifestNoop);
    }
    Ok(UpgradeApplyOutcome::Blocked {
        reason: UpgradeApplyBlockedReason::PreviewNotApplicable,
    })
}

/// Structured outcome for web/API witnesses that need the same observable
/// upgrade report without shelling out to the CLI binary.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct UpgradeWitnessRun {
    /// Canonical stdout lines that mirror `tanren-cli upgrade` output.
    pub stdout_lines: Vec<String>,
    /// Structured preview report with typed fields.
    pub preview: UpgradePreviewReport,
    /// Structured apply outcome (absent when confirm=false).
    pub apply_outcome: Option<UpgradeApplyOutcome>,
    /// Whether the overall run succeeded.
    pub success: bool,
}

impl UpgradeWitnessRun {
    /// Join `stdout_lines` into a newline-delimited payload.
    #[must_use]
    pub fn stdout(&self) -> String {
        self.stdout_lines.join("\n")
    }
}

/// Run `upgrade` in-process and render the same machine-readable witness lines
/// the CLI prints to stdout, alongside structured typed fields.
pub fn run_upgrade_witness(
    repository: &Path,
    confirm: bool,
) -> Result<UpgradeWitnessRun, InstallError> {
    let preview = preview_upgrade(repository)?;
    let mut lines = Vec::new();
    let report = preview.report();

    lines.push(format!(
        "status=preview command=upgrade repo={} preview_id={} changed={} destructive={} preserved={} concerns={}",
        display_repository_argument(repository),
        report.preview_id(),
        report.changed().len(),
        report.destructive().len(),
        report.preserved().len(),
        report.compatibility_concerns().len(),
    ));
    lines.push(format!(
        "preview changed=[{}] destructive=[{}] restored=[{}] removed=[{}] preserved=[{}] concerns=[{}]",
        format_encoded_path_list(report.changed()),
        format_encoded_path_list(report.destructive()),
        format_encoded_path_list(report.restored()),
        format_encoded_path_list(report.removed()),
        format_encoded_path_list(report.preserved()),
        report.compatibility_concerns().iter().map(UpgradeCompatibilityConcern::as_code).collect::<Vec<_>>().join(","),
    ));

    if !confirm {
        lines.push(format!(
            "status=confirmation_required command=upgrade repo={} confirm=false applied=false writes=0 removals=0 preserved=0 can_apply={}",
            display_repository_argument(repository),
            preview.can_apply(),
        ));
        return Ok(UpgradeWitnessRun {
            stdout_lines: lines,
            preview: report.clone(),
            apply_outcome: None,
            success: true,
        });
    }

    let apply_outcome = apply_upgrade(&preview, UpgradeApplyConfirmation::confirmed())?;

    match &apply_outcome {
        UpgradeApplyOutcome::NoManifestNoop => {
            lines.push(format!(
                "status=noop command=upgrade repo={} preview_id={} confirm=true applied=false outcome={} created=0 updated=0 removed=0 restored=0 preserved=0",
                display_repository_argument(repository),
                preview.report().preview_id(),
                UpgradeApplyOutcome::NoManifestNoop.label(),
            ));
        }
        UpgradeApplyOutcome::Applied { report } => {
            lines.push(format!(
                "status=ok command=upgrade repo={} preview_id={} confirm=true applied=true outcome={} created={} updated={} removed={} restored={} preserved={}",
                display_repository_argument(repository),
                preview.report().preview_id(),
                UpgradeApplyOutcome::APPLIED_LABEL,
                report.created.len(),
                report.updated.len(),
                report.removed.len(),
                report.restored.len(),
                report.preserved.len(),
            ));
            lines.push(format!(
                "applied created=[{}] updated=[{}] removed=[{}] restored=[{}] preserved=[{}]",
                format_encoded_path_list_from_str(&report.created),
                format_encoded_path_list_from_str(&report.updated),
                format_encoded_path_list_from_str(&report.removed),
                format_encoded_path_list_from_str(&report.restored),
                format_encoded_path_list_from_str(&report.preserved),
            ));
        }
        UpgradeApplyOutcome::Blocked { reason } => {
            lines.push(format!(
                "status=blocked command=upgrade repo={} preview_id={} confirm=true applied=false outcome={} reason={} created=0 updated=0 removed=0 restored=0 preserved=0",
                display_repository_argument(repository),
                preview.report().preview_id(),
                UpgradeApplyOutcome::Blocked { reason: *reason }.label(),
                reason.as_str(),
            ));
        }
    }

    Ok(UpgradeWitnessRun {
        stdout_lines: lines,
        preview: preview.report().clone(),
        apply_outcome: Some(apply_outcome),
        success: true,
    })
}

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}

fn format_encoded_path_list_from_str(paths: &[String]) -> String {
    paths
        .iter()
        .map(|p| encode_field(p.as_str()))
        .collect::<Vec<_>>()
        .join(",")
}
