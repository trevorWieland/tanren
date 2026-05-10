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

    pub(crate) fn assert_uninstall_preview_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=ok command=uninstall phase=preview")?;
        for field in ["remove=", "preserve=", "warning=", "nothing_to_uninstall="] {
            ensure_stdout_contains(run, field)?;
        }
        ensure_stdout_contains(run, "paths remove=[")?;
        ensure_stdout_contains(run, "preserve=[")?;
        ensure_stdout_contains(run, "warning=[")?;
        Ok(())
    }

    pub(crate) fn assert_uninstall_apply_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=ok command=uninstall phase=apply")?;
        for field in [
            "removed_generated=",
            "removed_metadata=",
            "nothing_to_uninstall=",
        ] {
            ensure_stdout_contains(run, field)?;
        }
        ensure_stdout_contains(run, "paths removed_generated=[")?;
        ensure_stdout_contains(run, "removed_metadata=[")?;
        Ok(())
    }

    pub(crate) fn assert_uninstall_preview_has_removals(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let remove_count = parse_status_count(&run.stdout, "remove").ok_or_else(|| {
            InstallStepError::StdoutMissingExpected {
                expected: "remove=<count>".to_owned(),
                stdout: run.stdout.clone(),
            }
        })?;
        if remove_count == 0 {
            return Err(InstallStepError::UninstallPreviewExpectedRemovals {
                stdout: run.stdout.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_uninstall_nothing_to_uninstall(
        &self,
        expected: bool,
    ) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let parsed = parse_status_bool(&run.stdout, "nothing_to_uninstall").ok_or_else(|| {
            InstallStepError::StdoutMissingExpected {
                expected: "nothing_to_uninstall=<bool>".to_owned(),
                stdout: run.stdout.clone(),
            }
        })?;
        if parsed != expected {
            return Err(InstallStepError::StdoutMissingExpected {
                expected: format!("nothing_to_uninstall={expected}"),
                stdout: run.stdout.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_uninstall_nothing_reason(&self, expected: &str) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let parsed = parse_status_value(&run.stdout, "nothing_reason").ok_or_else(|| {
            InstallStepError::StdoutMissingExpected {
                expected: "nothing_reason=<value>".to_owned(),
                stdout: run.stdout.clone(),
            }
        })?;
        if parsed != expected {
            return Err(InstallStepError::StdoutMissingExpected {
                expected: format!("nothing_reason={expected}"),
                stdout: run.stdout.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_uninstall_preview_lists_removal_path(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let remove_entries = parse_paths_segment(&run.stdout, "remove").ok_or_else(|| {
            InstallStepError::StdoutMissingExpected {
                expected: "paths remove=[...]".to_owned(),
                stdout: run.stdout.clone(),
            }
        })?;
        let has_path = split_csv(remove_entries)
            .into_iter()
            .any(|entry| entry == relative_path.as_str());
        if !has_path {
            return Err(InstallStepError::StdoutMissingExpected {
                expected: format!("remove path '{}'", relative_path.as_str()),
                stdout: run.stdout.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_uninstall_preview_lists_preserved_path(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let preserve_entries = parse_paths_segment(&run.stdout, "preserve").ok_or_else(|| {
            InstallStepError::StdoutMissingExpected {
                expected: "paths preserve=[...]".to_owned(),
                stdout: run.stdout.clone(),
            }
        })?;
        let has_path = split_csv(preserve_entries).into_iter().any(|entry| {
            entry
                .split_once(':')
                .is_some_and(|(path, _reason)| path == relative_path.as_str())
        });
        if !has_path {
            return Err(InstallStepError::StdoutMissingExpected {
                expected: format!("preserve path '{}'", relative_path.as_str()),
                stdout: run.stdout.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_uninstall_apply_lists_removed_generated_path(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let removed_entries =
            parse_paths_segment(&run.stdout, "removed_generated").ok_or_else(|| {
                InstallStepError::StdoutMissingExpected {
                    expected: "paths removed_generated=[...]".to_owned(),
                    stdout: run.stdout.clone(),
                }
            })?;
        let has_path = split_csv(removed_entries)
            .into_iter()
            .any(|entry| entry == relative_path.as_str());
        if !has_path {
            return Err(InstallStepError::StdoutMissingExpected {
                expected: format!("removed_generated path '{}'", relative_path.as_str()),
                stdout: run.stdout.clone(),
            });
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
        let _run = self.require_last_run()?;
        if matches!(
            self.last_command_kind,
            Some(
                super::context::InstallCommandKind::UninstallPreview
                    | super::context::InstallCommandKind::UninstallApply
            )
        ) {
            return self.assert_uninstall_no_install_leaves_repository_snapshot_unchanged();
        }

        let before = self
            .snapshot_before_last_run
            .as_ref()
            .ok_or(InstallStepError::MissingSnapshotBeforeRun)?;
        let after =
            crate::steps::install_snapshot::RepositorySnapshot::capture(&self.repository_root)?;
        if &after != before {
            let mut changed_paths = before.changed_paths(&after);
            if changed_paths.len() > 20 {
                changed_paths.truncate(20);
                changed_paths.push("...truncated...".to_owned());
            }
            return Err(InstallStepError::RepositorySnapshotMismatch {
                repository_root: self.repository_root.display().to_string(),
                changed_paths,
            });
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

    pub(crate) fn assert_uninstall_removed_generated_assets_and_manifest(
        &self,
    ) -> InstallStepResult<()> {
        manifest_helpers::assert_uninstall_removes_generated_assets_and_manifest(
            &self.repository_root,
        )
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

fn parse_status_count(stdout: &str, field: &str) -> Option<usize> {
    parse_status_value(stdout, field).and_then(|raw| raw.parse::<usize>().ok())
}

fn parse_status_bool(stdout: &str, field: &str) -> Option<bool> {
    parse_status_value(stdout, field).and_then(|raw| raw.parse::<bool>().ok())
}

fn parse_status_value<'a>(stdout: &'a str, field: &str) -> Option<&'a str> {
    let prefix = format!("{field}=");
    stdout
        .lines()
        .find(|line| line.starts_with("status=ok command=uninstall"))
        .and_then(|line| {
            line.split_whitespace()
                .find_map(|segment| segment.strip_prefix(&prefix))
        })
}

fn parse_paths_segment<'a>(stdout: &'a str, key: &str) -> Option<&'a str> {
    let marker = format!("{key}=[");
    stdout
        .lines()
        .find(|line| line.starts_with("paths "))
        .and_then(|line| {
            let start = line.find(&marker)?;
            let remainder = &line[start + marker.len()..];
            let end = remainder.find(']')?;
            Some(&remainder[..end])
        })
}

fn split_csv(list: &str) -> Vec<&str> {
    if list.is_empty() {
        return Vec::new();
    }
    list.split(',').filter(|entry| !entry.is_empty()).collect()
}
