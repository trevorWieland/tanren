//! CLI adapter for `tanren-cli install`.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::install::error::InstallCommandError;
use crate::install::manifest::RepoRelativePath;
use crate::install::{
    InstallReport, UninstallApplyReport, UninstallPreserveReason, UninstallPreview,
    UninstallWarning, UninstallWarningKind, apply_install, apply_uninstall, plan_uninstall,
};

/// `tanren-cli install` arguments.
#[derive(Debug, Clone, Args)]
pub struct InstallCommand {
    /// Repository path to install into (defaults to current directory).
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    /// Standards profile to install.
    #[arg(long)]
    profile: String,
    /// Comma-separated integration names (defaults to all).
    #[arg(long)]
    integrations: Option<String>,
}

impl InstallCommand {
    /// Validate install inputs, apply install, and emit a concise outcome report.
    pub fn run(&self) -> Result<(), InstallCommandError> {
        let report = apply_install(&self.repo, &self.profile, self.integrations.as_deref())
            .map_err(InstallCommandError::from)?;
        self.write_success_report(&report)
    }

    fn write_success_report(&self, report: &InstallReport) -> Result<(), InstallCommandError> {
        let repository = display_repository_argument(&self.repo);
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status=ok command=install repo={} created={} updated={} removed={} restored={} preserved={}",
            repository,
            report.created.len(),
            report.updated.len(),
            report.removed.len(),
            report.restored.len(),
            report.preserved.len(),
        )
        .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        writeln!(
            handle,
            "paths created=[{}] updated=[{}] removed=[{}] restored=[{}] preserved=[{}]",
            format_path_list(&report.created),
            format_path_list(&report.updated),
            format_path_list(&report.removed),
            format_path_list(&report.restored),
            format_path_list(&report.preserved),
        )
        .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        Ok(())
    }
}

/// `tanren-cli uninstall` arguments.
#[derive(Debug, Clone, Args)]
pub struct UninstallCommand {
    /// Repository path to uninstall from (defaults to current directory).
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    /// Apply removals after preview; omitted means preview-only.
    #[arg(long, default_value_t = false)]
    confirm: bool,
}

impl UninstallCommand {
    /// Build preview, optionally apply it, and emit deterministic reports.
    pub fn run(&self) -> Result<(), InstallCommandError> {
        let preview =
            plan_uninstall(&self.repo).map_err(InstallCommandError::from_uninstall_error)?;
        self.write_preview_report(&preview)?;
        if !self.confirm {
            return Ok(());
        }

        let report = apply_uninstall(&self.repo, &preview)
            .map_err(InstallCommandError::from_uninstall_error)?;
        self.write_apply_report(&preview, &report)
    }

    fn write_preview_report(&self, preview: &UninstallPreview) -> Result<(), InstallCommandError> {
        let repository = display_repository_argument(&self.repo);
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status=ok command=uninstall phase=preview repo={} remove={} preserve={} warning={} nothing_to_uninstall={}",
            repository,
            preview.remove().len(),
            preview.preserve().len(),
            preview.warning().len(),
            preview.nothing_to_uninstall(),
        )
        .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        writeln!(
            handle,
            "paths remove=[{}] preserve=[{}] warning=[{}]",
            format_path_list(preview.remove()),
            format_preserved_list(preview.preserve()),
            format_warning_list(preview.warning()),
        )
        .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        Ok(())
    }

    fn write_apply_report(
        &self,
        preview: &UninstallPreview,
        report: &UninstallApplyReport,
    ) -> Result<(), InstallCommandError> {
        let repository = display_repository_argument(&self.repo);
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status=ok command=uninstall phase=apply repo={} removed_generated={} removed_metadata={} nothing_to_uninstall={}",
            repository,
            report.removed_generated.len(),
            report.removed_metadata.len(),
            preview.nothing_to_uninstall(),
        )
        .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        writeln!(
            handle,
            "paths removed_generated=[{}] removed_metadata=[{}]",
            format_path_list(&report.removed_generated),
            format_path_list(&report.removed_metadata),
        )
        .map_err(|source| InstallCommandError::StdoutWriteFailure { source })?;
        Ok(())
    }
}

fn format_path_list(paths: &[RepoRelativePath]) -> String {
    paths
        .iter()
        .map(RepoRelativePath::as_str)
        .collect::<Vec<_>>()
        .join(",")
}

fn format_preserved_list(paths: &[crate::install::UninstallPreservedPath]) -> String {
    paths
        .iter()
        .map(|entry| {
            format!(
                "{}:{}",
                entry.path().as_str(),
                preserve_reason_name(entry.reason())
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn format_warning_list(warnings: &[UninstallWarning]) -> String {
    warnings
        .iter()
        .map(|warning| match warning.path() {
            Some(path) => format!("{}:{}", warning_kind_name(warning.kind()), path.as_str()),
            None => warning_kind_name(warning.kind()).to_owned(),
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn preserve_reason_name(reason: UninstallPreserveReason) -> &'static str {
    match reason {
        UninstallPreserveReason::NotReplaceGenerated => "not_replace_generated",
        UninstallPreserveReason::UntrustedManifestEntry => "untrusted_manifest_entry",
        UninstallPreserveReason::ContentDrifted => "content_drifted",
        UninstallPreserveReason::MissingFromRepository => "missing_from_repository",
        UninstallPreserveReason::UnsafeRepositoryPath => "unsafe_repository_path",
    }
}

fn warning_kind_name(kind: UninstallWarningKind) -> &'static str {
    match kind {
        UninstallWarningKind::ManifestMissing => "manifest_missing",
        UninstallWarningKind::UntrustedManifestEntry => "untrusted_manifest_entry",
        UninstallWarningKind::UnsafeRepositoryPath => "unsafe_repository_path",
        UninstallWarningKind::ContentDrifted => "content_drifted",
    }
}

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}
