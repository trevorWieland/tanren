//! CLI adapter for `tanren-cli upgrade`.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use super::error::UpgradeCommandError;
use tanren_delivery::install::{
    UpgradeApplyConfirmation, UpgradeApplyOutcome, UpgradePreview, UpgradePreviewReport,
    apply_upgrade, preview_upgrade,
};
use tanren_delivery::install::{encode_field, format_encoded_path_list};

/// `tanren-cli upgrade` arguments.
#[derive(Debug, Clone, Args)]
pub struct UpgradeCommand {
    /// Repository path to upgrade (defaults to current directory).
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    /// Apply the previewed upgrade plan.
    #[arg(long, default_value_t = false)]
    confirm: bool,
}

impl UpgradeCommand {
    /// Validate inputs, emit preview, and optionally apply the upgrade plan.
    pub fn run(&self) -> Result<(), UpgradeCommandError> {
        let preview = preview_upgrade(&self.repo).map_err(UpgradeCommandError::from)?;
        let preview_report = preview.report();
        self.write_preview_report(preview_report)?;

        if !self.confirm {
            return self.write_confirmation_required(&preview);
        }

        let apply_outcome = apply_upgrade(&preview, UpgradeApplyConfirmation::confirmed())
            .map_err(UpgradeCommandError::from)?;
        self.write_apply_result(&apply_outcome)
    }

    fn write_preview_report(
        &self,
        report: &UpgradePreviewReport,
    ) -> Result<(), UpgradeCommandError> {
        let repository = display_repository_argument(&self.repo);
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status=preview command=upgrade repo={} changed={} destructive={} preserved={} concerns={}",
            repository,
            report.changed().len(),
            report.destructive().len(),
            report.preserved().len(),
            report.compatibility_concerns().len(),
        )
        .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source })?;
        writeln!(
            handle,
            "preview changed=[{}] destructive=[{}] restored=[{}] removed=[{}] preserved=[{}] concerns=[{}]",
            format_encoded_path_list(report.changed()),
            format_encoded_path_list(report.destructive()),
            format_encoded_path_list(report.restored()),
            format_encoded_path_list(report.removed()),
            format_encoded_path_list(report.preserved()),
            report.compatibility_concerns().iter().map(tanren_delivery::install::UpgradeCompatibilityConcern::as_code).collect::<Vec<_>>().join(","),
        )
        .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source })?;
        Ok(())
    }

    fn write_confirmation_required(
        &self,
        preview: &UpgradePreview,
    ) -> Result<(), UpgradeCommandError> {
        let repository = display_repository_argument(&self.repo);
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let apply_possible = preview.can_apply();
        writeln!(
            handle,
            "status=confirmation_required command=upgrade repo={repository} confirm=false applied=false writes=0 removals=0 preserved=0 can_apply={apply_possible}",
        )
        .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source })
    }

    fn write_apply_result(
        &self,
        apply_outcome: &UpgradeApplyOutcome,
    ) -> Result<(), UpgradeCommandError> {
        let repository = display_repository_argument(&self.repo);
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        match apply_outcome {
            UpgradeApplyOutcome::NoManifestNoop => writeln!(
                handle,
                "status=noop command=upgrade repo={repository} confirm=true applied=false outcome=no_manifest_noop created=0 updated=0 removed=0 restored=0 preserved=0",
            )
            .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source }),
            UpgradeApplyOutcome::Applied { report } => {
                writeln!(
                    handle,
                    "status=ok command=upgrade repo={} confirm=true applied=true outcome=applied created={} updated={} removed={} restored={} preserved={}",
                    repository,
                    report.created.len(),
                    report.updated.len(),
                    report.removed.len(),
                    report.restored.len(),
                    report.preserved.len(),
                )
                .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source })?;
                writeln!(
                    handle,
                    "applied created=[{}] updated=[{}] removed=[{}] restored=[{}] preserved=[{}]",
                    format_encoded_path_list_from_str(&report.created),
                    format_encoded_path_list_from_str(&report.updated),
                    format_encoded_path_list_from_str(&report.removed),
                    format_encoded_path_list_from_str(&report.restored),
                    format_encoded_path_list_from_str(&report.preserved),
                )
                .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source })
            }
            UpgradeApplyOutcome::Blocked { reason } => writeln!(
                handle,
                "status=blocked command=upgrade repo={} confirm=true applied=false outcome=blocked reason={} created=0 updated=0 removed=0 restored=0 preserved=0",
                repository,
                reason.as_str(),
            )
            .map_err(|source| UpgradeCommandError::StdoutWriteFailure { source }),
        }
    }
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
