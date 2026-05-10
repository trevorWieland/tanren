use std::fs;

use crate::steps::install::context::InstallCommandOutcome;
use tanren_testkit::{DriftCommandStatus, DriftOutput, DriftPathStatus, parse_drift_output};

use super::context::InstallContext;
use super::manifest_helpers;
use super::manifest_helpers::RepositoryRelativePath;
use super::projection_authority::{
    PathExpectation, assert_path_projection, assert_typed_zero_writes,
};
use super::{InstallStepError, InstallStepResult};

impl InstallContext {
    pub(crate) fn assert_success(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if !run.is_success() {
            return Err(InstallStepError::InstallCommandExpectedSuccess {
                diagnostic: run.redacted_diagnostic(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_nonzero(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if run.is_success() {
            return Err(InstallStepError::InstallCommandExpectedFailure {
                diagnostic: run.redacted_diagnostic(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_drift_success(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if !run.is_success() {
            return Err(InstallStepError::DriftCommandExpectedSuccess {
                diagnostic: run.redacted_diagnostic(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_drift_nonzero(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if run.is_success() {
            return Err(InstallStepError::DriftCommandExpectedFailure {
                diagnostic: run.redacted_diagnostic(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_summary_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=ok command=install")?;
        for field in &[
            "created=",
            "updated=",
            "removed=",
            "restored=",
            "preserved=",
        ] {
            ensure_stdout_contains(run, field)?;
        }
        for section in &[
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
        if !run.stderr().contains("error: validation_failed") {
            return Err(InstallStepError::ValidationFailureMissing {
                diagnostic: run.redacted_diagnostic(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_drift_output_reports_no_drift(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let output = parse_drift_output_or_err(run)?;
        if output.status() != DriftCommandStatus::Ok {
            return Err(InstallStepError::DriftCommandExpectedSuccess {
                diagnostic: run.redacted_diagnostic(),
            });
        }
        for (label, count) in [
            ("drift=0", output.drift_count()),
            ("changed_generated=0", output.changed_generated_count()),
            ("missing_generated=0", output.missing_generated_count()),
            ("missing_preserved=0", output.missing_preserved_count()),
        ] {
            if count != 0 {
                return Err(InstallStepError::StdoutMissingExpected {
                    expected: label.to_owned(),
                    diagnostic: run.redacted_diagnostic(),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn assert_drift_output_reports_generated_asset_drift(
        &self,
        rp: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        self.assert_drift_detail(
            DriftCommandStatus::Drift,
            DriftPathStatus::ChangedGenerated,
            rp,
        )
    }

    pub(crate) fn assert_drift_output_reports_missing_generated_asset(
        &self,
        rp: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        self.assert_drift_detail(
            DriftCommandStatus::Drift,
            DriftPathStatus::MissingGenerated,
            rp,
        )
    }

    pub(crate) fn assert_drift_output_reports_missing_preserved_standard(
        &self,
        rp: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        self.assert_drift_detail(
            DriftCommandStatus::Drift,
            DriftPathStatus::MissingPreserved,
            rp,
        )
    }

    pub(crate) fn assert_drift_output_reports_accepted_preserved_edit(
        &self,
        rp: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        self.assert_drift_detail(
            DriftCommandStatus::Ok,
            DriftPathStatus::AcceptedPreserved,
            rp,
        )
    }

    fn assert_drift_detail(
        &self,
        expected_status: DriftCommandStatus,
        expected_path_status: DriftPathStatus,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let output = parse_drift_output_or_err(run)?;
        ensure_drift_status(&output, expected_status, run)?;
        ensure_detail_contains(&output, expected_path_status, relative_path.as_str(), run)
    }

    pub(crate) fn assert_no_absolute_repository_path_leaked(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        for line in run.stdout().lines().chain(run.stderr().lines()) {
            if line.contains("/tmp/") || line.contains("/home/") {
                return Err(InstallStepError::OutputLeakedAbsoluteRepositoryPath {
                    diagnostic: run.redacted_diagnostic(),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn assert_stderr_contains(&self, expected: &str) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if !run.stderr().contains(expected) {
            return Err(InstallStepError::StderrMissingExpected {
                expected: expected.to_owned(),
                diagnostic: run.redacted_diagnostic(),
            });
        }
        Ok(())
    }

    /// Assert no files written since last run.
    /// Primary: typed projection/drift output. Secondary: snapshot comparison.
    pub(crate) fn assert_no_writes_since_last_run(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        assert_typed_zero_writes(run, self.require_last_command_kind()?)?;
        let before = self
            .fixture_proof_snapshot_before_last_run
            .as_ref()
            .ok_or(InstallStepError::MissingSnapshotBeforeRun)?;
        let after =
            crate::steps::install_snapshot::RepositorySnapshot::capture(&self.repository_root)?;
        if before != &after {
            return Err(InstallStepError::RepositorySnapshotMismatch);
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

    /// Assert file preserves baseline. Primary: projection. Secondary: filesystem.
    pub(crate) fn assert_file_content_preserved(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        assert_path_projection(self, relative_path, PathExpectation::Preserved)?;
        let baseline = self.require_baseline(relative_path)?;
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

    /// Assert file replaced from baseline. Primary: projection. Secondary: filesystem.
    pub(crate) fn assert_file_content_replaced(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        assert_path_projection(self, relative_path, PathExpectation::Replaced)?;
        let baseline = self.require_baseline(relative_path)?;
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

    fn require_baseline(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<&Vec<u8>> {
        self.fixture_proof_file_baselines
            .get(relative_path)
            .ok_or_else(|| InstallStepError::MissingBaseline {
                path: relative_path.as_str().to_owned(),
            })
    }
}

fn ensure_stdout_contains(run: &InstallCommandOutcome, expected: &str) -> InstallStepResult<()> {
    if !run.stdout().contains(expected) {
        return Err(InstallStepError::StdoutMissingExpected {
            expected: expected.to_owned(),
            diagnostic: run.redacted_diagnostic(),
        });
    }
    Ok(())
}

fn parse_drift_output_or_err(run: &InstallCommandOutcome) -> InstallStepResult<DriftOutput> {
    parse_drift_output(run.stdout()).map_err(|_| InstallStepError::StdoutMissingExpected {
        expected: "valid drift output".to_owned(),
        diagnostic: run.redacted_diagnostic(),
    })
}

fn ensure_drift_status(
    output: &DriftOutput,
    expected: DriftCommandStatus,
    run: &InstallCommandOutcome,
) -> InstallStepResult<()> {
    if output.status() != expected {
        return Err(InstallStepError::StdoutMissingExpected {
            expected: format!(
                "status={}",
                match expected {
                    DriftCommandStatus::Ok => "ok",
                    DriftCommandStatus::Drift => "drift",
                }
            ),
            diagnostic: run.redacted_diagnostic(),
        });
    }
    Ok(())
}

fn ensure_detail_contains(
    output: &DriftOutput,
    expected_status: DriftPathStatus,
    path: &str,
    run: &InstallCommandOutcome,
) -> InstallStepResult<()> {
    if !output
        .details()
        .iter()
        .any(|r| r.status() == expected_status && r.path() == path)
    {
        let label = match expected_status {
            DriftPathStatus::ChangedGenerated => "changed_generated",
            DriftPathStatus::MissingGenerated => "missing_generated",
            DriftPathStatus::MissingPreserved => "missing_preserved",
            DriftPathStatus::AcceptedPreserved => "accepted_preserved",
            DriftPathStatus::Clean => "clean",
        };
        return Err(InstallStepError::StdoutMissingExpected {
            expected: format!("detail status={label} path={path}"),
            diagnostic: run.redacted_diagnostic(),
        });
    }
    Ok(())
}
