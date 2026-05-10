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

    pub(crate) fn assert_drift_success(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if !run.success {
            return Err(InstallStepError::DriftCommandExpectedSuccess {
                status: run.status_code,
                stdout: run.stdout.clone(),
                stderr: run.stderr.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_drift_nonzero(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if run.success {
            return Err(InstallStepError::DriftCommandExpectedFailure {
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

    pub(crate) fn assert_drift_output_reports_no_drift(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        for expected in [
            "status=ok command=drift",
            "drift=0",
            "changed_generated=0",
            "missing_generated=0",
            "missing_preserved=0",
        ] {
            ensure_stdout_contains(run, expected)?;
        }
        Ok(())
    }

    pub(crate) fn assert_drift_output_reports_generated_asset_drift(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=drift command=drift")?;
        ensure_stdout_list_field_contains(run, DriftListField::ChangedGenerated, relative_path)
    }

    pub(crate) fn assert_drift_output_reports_missing_generated_asset(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=drift command=drift")?;
        ensure_stdout_list_field_contains(run, DriftListField::MissingGenerated, relative_path)
    }

    pub(crate) fn assert_drift_output_reports_missing_preserved_standard(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=drift command=drift")?;
        ensure_stdout_list_field_contains(run, DriftListField::MissingPreserved, relative_path)
    }

    pub(crate) fn assert_drift_output_reports_accepted_preserved_edit(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=ok command=drift")?;
        ensure_stdout_list_field_contains(run, DriftListField::AcceptedPreserved, relative_path)
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

    pub(crate) fn assert_file_exists(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path);
        if !absolute.exists() {
            return Err(InstallStepError::ExpectedFileToExist { path: absolute });
        }
        Ok(())
    }

    pub(crate) fn assert_file_absent(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path);
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
        let absolute = self.repository_path(relative_path);
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
        let absolute = self.repository_path(relative_path);
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
        let absolute = self.repository_path(relative_path);
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

fn ensure_stdout_list_field_contains(
    run: &InstallCommandOutcome,
    field: DriftListField,
    path: &RepositoryRelativePath,
) -> InstallStepResult<()> {
    let field_name = field.as_str();
    let marker = format!("{field_name}=[");
    let value_start = run.stdout.find(marker.as_str()).ok_or_else(|| {
        InstallStepError::StdoutMissingExpected {
            expected: marker.clone(),
            stdout: run.stdout.clone(),
        }
    })?;
    let items_start = value_start + marker.len();
    let remainder = &run.stdout[items_start..];
    let closing = remainder
        .find(']')
        .ok_or_else(|| InstallStepError::StdoutMissingExpected {
            expected: format!("{field_name} list closing bracket"),
            stdout: run.stdout.clone(),
        })?;
    let list = &remainder[..closing];
    if !list.split(',').any(|item| item == path.as_str()) {
        return Err(InstallStepError::StdoutMissingExpected {
            expected: format!("{field_name} contains {}", path.as_str()),
            stdout: run.stdout.clone(),
        });
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum DriftListField {
    ChangedGenerated,
    MissingGenerated,
    MissingPreserved,
    AcceptedPreserved,
}

impl DriftListField {
    fn as_str(self) -> &'static str {
        match self {
            Self::ChangedGenerated => "changed_generated",
            Self::MissingGenerated => "missing_generated",
            Self::MissingPreserved => "missing_preserved",
            Self::AcceptedPreserved => "accepted_preserved",
        }
    }
}
