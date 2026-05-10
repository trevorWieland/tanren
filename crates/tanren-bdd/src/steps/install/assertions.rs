use std::fs;

use crate::steps::install::context::InstallCommandOutcome;

use super::context::InstallContext;
use super::manifest_helpers;
use super::manifest_helpers::RepositoryRelativePath;
use super::{InstallStepError, InstallStepResult};

impl InstallContext {
    pub(crate) fn assert_success(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if !run.success {
            return Err(InstallStepError::InstallCommandExpectedSuccess {
                status: run.status_code,
                stdout: run.stdout.clone(),
                stderr: run.stderr.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_nonzero(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if run.success {
            return Err(InstallStepError::InstallCommandExpectedFailure {
                stdout: run.stdout.clone(),
                stderr: run.stderr.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_summary_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=ok command=install")?;
        for field in [
            "created=",
            "updated=",
            "removed=",
            "restored=",
            "preserved=",
        ] {
            ensure_stdout_contains(run, field)?;
        }
        for section in [
            "paths created=[",
            "updated=[",
            "removed=[",
            "restored=[",
            "preserved=[",
        ] {
            ensure_stdout_contains(run, section)?;
        }
        Ok(())
    }

    pub(crate) fn assert_validation_failure_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if !run.stderr.contains("error: validation_failed") {
            return Err(InstallStepError::ValidationFailureMissing {
                stderr: run.stderr.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_no_writes_since_last_run(&self) -> InstallStepResult<()> {
        let before = self
            .snapshot_before_last_run
            .as_ref()
            .ok_or(InstallStepError::MissingSnapshotBeforeRun)?;
        let after =
            crate::steps::install_snapshot::RepositorySnapshot::capture(&self.repository_root)?;
        if &after != before {
            return Err(InstallStepError::RepositorySnapshotMismatch);
        }
        Ok(())
    }

    pub(crate) fn assert_repository_matches_labeled_snapshot(
        &self,
        label: &str,
    ) -> InstallStepResult<()> {
        let expected = self.labeled_snapshots.get(label).ok_or_else(|| {
            InstallStepError::MissingLabeledSnapshot {
                label: label.to_owned(),
            }
        })?;
        let actual =
            crate::steps::install_snapshot::RepositorySnapshot::capture(&self.repository_root)?;
        if &actual != expected {
            return Err(InstallStepError::RepositorySnapshotMismatch);
        }
        Ok(())
    }

    pub(crate) fn assert_stderr_contains(&self, expected: &str) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if !run.stderr.contains(expected) {
            return Err(InstallStepError::StderrMissingExpected {
                expected: expected.to_owned(),
                stderr: run.stderr.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_no_absolute_repository_path_leaked(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let repository_path = self.repository_root.display().to_string();
        if run.stdout.contains(&repository_path) || run.stderr.contains(&repository_path) {
            return Err(InstallStepError::OutputLeakedAbsoluteRepositoryPath {
                path: repository_path,
                stdout: run.stdout.clone(),
                stderr: run.stderr.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_upgrade_preview_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=preview command=upgrade")?;
        ensure_stdout_contains(run, "preview changed=[")?;
        ensure_stdout_contains(run, "destructive=[")?;
        ensure_stdout_contains(run, "preserved=[")?;
        ensure_stdout_contains(run, "concerns=[")?;
        Ok(())
    }

    pub(crate) fn assert_upgrade_confirmation_required_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=confirmation_required command=upgrade")?;
        ensure_stdout_contains(run, "confirm=false applied=false")?;
        Ok(())
    }

    pub(crate) fn assert_upgrade_apply_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=ok command=upgrade")?;
        ensure_stdout_contains(run, "confirm=true applied=true")?;
        ensure_stdout_contains(run, "applied created=[")?;
        ensure_stdout_contains(run, "updated=[")?;
        ensure_stdout_contains(run, "removed=[")?;
        ensure_stdout_contains(run, "restored=[")?;
        ensure_stdout_contains(run, "preserved=[")?;
        Ok(())
    }

    pub(crate) fn assert_upgrade_noop_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=noop command=upgrade")?;
        ensure_stdout_contains(run, "confirm=true applied=false")?;
        Ok(())
    }

    pub(crate) fn assert_upgrade_preview_contains_concern(
        &self,
        concern: &str,
    ) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, concern)
    }

    pub(crate) fn assert_upgrade_preview_contains_path(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, relative_path.as_str())
    }

    pub(crate) fn assert_file_exists(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path.as_str())?;
        if !absolute.exists() {
            return Err(InstallStepError::ExpectedFileToExist { path: absolute });
        }
        Ok(())
    }

    pub(crate) fn assert_file_absent(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path.as_str())?;
        if absolute.exists() {
            return Err(InstallStepError::ExpectedFileToBeAbsent { path: absolute });
        }
        Ok(())
    }

    pub(crate) fn assert_exact_file_content(
        &self,
        relative_path: &RepositoryRelativePath,
        expected: String,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path.as_str())?;
        let bytes = fs::read(&absolute).map_err(|source| InstallStepError::ReadFile {
            path: absolute.clone(),
            action: "read repository fixture file",
            source,
        })?;
        if bytes != expected.into_bytes() {
            return Err(InstallStepError::UnexpectedFileContent { path: absolute });
        }
        Ok(())
    }

    pub(crate) fn assert_file_content_preserved(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let baseline =
            self.baselines
                .get(relative_path)
                .ok_or_else(|| InstallStepError::MissingBaseline {
                    path: relative_path.as_str().to_owned(),
                })?;
        let absolute = self.repository_path(relative_path.as_str())?;
        let bytes = fs::read(&absolute).map_err(|source| InstallStepError::ReadFile {
            path: absolute.clone(),
            action: "read repository fixture file",
            source,
        })?;
        if &bytes != baseline {
            return Err(InstallStepError::FileContentChanged { path: absolute });
        }
        Ok(())
    }

    pub(crate) fn assert_file_content_replaced(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let baseline =
            self.baselines
                .get(relative_path)
                .ok_or_else(|| InstallStepError::MissingBaseline {
                    path: relative_path.as_str().to_owned(),
                })?;
        let absolute = self.repository_path(relative_path.as_str())?;
        let bytes = fs::read(&absolute).map_err(|source| InstallStepError::ReadFile {
            path: absolute.clone(),
            action: "read repository fixture file",
            source,
        })?;
        if &bytes == baseline {
            return Err(InstallStepError::FileContentNotReplaced { path: absolute });
        }
        Ok(())
    }

    pub(crate) fn assert_rust_cargo_default_assets_installed(&self) -> InstallStepResult<()> {
        manifest_helpers::assert_rust_cargo_default_assets_installed(&self.repository_root)
    }

    pub(crate) fn assert_rust_cargo_standards_installed(&self) -> InstallStepResult<()> {
        manifest_helpers::assert_rust_cargo_standards_installed(&self.repository_root)
    }

    pub(crate) fn assert_selected_integration_command_assets(
        &self,
        integrations: &str,
    ) -> InstallStepResult<()> {
        manifest_helpers::assert_selected_integration_command_assets(
            &self.repository_root,
            integrations,
        )
    }

    pub(crate) fn assert_manifest_rust_cargo_defaults(&self) -> InstallStepResult<()> {
        manifest_helpers::assert_manifest_rust_cargo_defaults(&self.repository_root)
    }
}

fn ensure_stdout_contains(run: &InstallCommandOutcome, expected: &str) -> InstallStepResult<()> {
    if !run.stdout.contains(expected) {
        return Err(InstallStepError::StdoutMissingExpected {
            expected: expected.to_owned(),
            stdout: run.stdout.clone(),
        });
    }
    Ok(())
}
