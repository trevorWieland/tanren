use std::fs;

use serde::Deserialize;
use serde_json::Value;
use tanren_configuration_secrets::{
    EffectiveConfigurationActorUsability, EffectiveConfigurationFreshness,
    EffectiveConfigurationMetadata, EffectiveConfigurationPolicyConstraint,
    EffectiveConfigurationResolutionKind, EffectiveConfigurationSettingFamily,
    EffectiveConfigurationSourceScope, StandardsRoot,
};

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

    pub(crate) fn assert_standards_inspect_report_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let report = parse_stdout_json_report(run)?;
        let project_methodology_config = self.load_project_methodology_config()?;
        if report.status != StandardsInspectReportStatus::Ok {
            return Err(InstallStepError::UnexpectedStandardsInspectStatus {
                actual: format!("{:?}", report.status),
            });
        }
        if report.command != StandardsInspectReportCommand::StandardsInspect {
            return Err(InstallStepError::UnexpectedStandardsInspectCommand {
                actual: format!("{:?}", report.command),
            });
        }
        let expected_profile = project_methodology_config.profile.as_str();
        if report.profile != expected_profile {
            return Err(InstallStepError::UnexpectedStandardsInspectProfile {
                expected: expected_profile.to_owned(),
                actual: report.profile,
            });
        }
        if report.repository.trim().is_empty() {
            return Err(InstallStepError::UnexpectedStandardsInspectRepositoryEmpty);
        }
        if report.standards_root.as_str().trim().is_empty() {
            return Err(InstallStepError::UnexpectedStandardsInspectStandardsRootEmpty);
        }
        if report.standards_root.as_str() != project_methodology_config.standards_root.as_str() {
            return Err(InstallStepError::UnexpectedStandardsInspectStandardsRoot {
                expected: project_methodology_config
                    .standards_root
                    .as_str()
                    .to_owned(),
                actual: report.standards_root.as_str().to_owned(),
            });
        }
        if report.standards_count == 0 {
            return Err(InstallStepError::UnexpectedStandardsInspectCountZero);
        }
        if report.first_standard_name.trim().is_empty() {
            return Err(InstallStepError::UnexpectedStandardsInspectFirstStandardNameEmpty);
        }
        let first_standard_path = report.first_standard_path.clone();
        RepositoryRelativePath::parse(first_standard_path.clone())?;
        let configured_standards_root_prefix =
            format!("{}/", project_methodology_config.standards_root.as_str());
        if !first_standard_path.starts_with(&configured_standards_root_prefix) {
            return Err(
                InstallStepError::UnexpectedStandardsInspectFirstStandardPathOutsideStandardsRoot {
                    path: first_standard_path,
                    standards_root: project_methodology_config
                        .standards_root
                        .as_str()
                        .to_owned(),
                },
            );
        }
        assert_effective_configuration_metadata(
            &report.effective_configuration.profile,
            EffectiveConfigurationSettingFamily::StandardsProfile,
        )?;
        assert_effective_configuration_metadata(
            &report.effective_configuration.standards_root,
            EffectiveConfigurationSettingFamily::StandardsRoot,
        )?;
        Ok(())
    }

    pub(crate) fn assert_standards_missing_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let project_methodology_config = self.load_project_methodology_config()?;
        for fragment in [
            "error: standards_missing -",
            "configured standards root is missing:",
            project_methodology_config.standards_root.as_str(),
        ] {
            if !run.stderr.contains(fragment) {
                return Err(InstallStepError::StderrMissingExpected {
                    expected: fragment.to_owned(),
                    stderr: run.stderr.clone(),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn assert_standards_parse_failure_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        let project_methodology_config = self.load_project_methodology_config()?;
        for fragment in [
            "error: standards_parse_failed -",
            "failed to parse standards frontmatter in",
            "missing opening frontmatter delimiter",
            project_methodology_config.standards_root.as_str(),
        ] {
            if !run.stderr.contains(fragment) {
                return Err(InstallStepError::StderrMissingExpected {
                    expected: fragment.to_owned(),
                    stderr: run.stderr.clone(),
                });
            }
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

fn parse_stdout_json_report(
    run: &InstallCommandOutcome,
) -> InstallStepResult<StandardsInspectReport> {
    let json: Value = serde_json::from_str(run.stdout.trim()).map_err(|source| {
        InstallStepError::StdoutJsonDecode {
            source,
            stdout: run.stdout.clone(),
        }
    })?;
    for path in [
        ["effective_configuration", "profile", "setting_family"],
        ["effective_configuration", "profile", "source_scope"],
        ["effective_configuration", "profile", "resolution_kind"],
        ["effective_configuration", "profile", "policy_constraint"],
        ["effective_configuration", "profile", "actor_usability"],
        ["effective_configuration", "profile", "freshness"],
        ["effective_configuration", "profile", "projection_position"],
        [
            "effective_configuration",
            "standards_root",
            "setting_family",
        ],
        ["effective_configuration", "standards_root", "source_scope"],
        [
            "effective_configuration",
            "standards_root",
            "resolution_kind",
        ],
        [
            "effective_configuration",
            "standards_root",
            "policy_constraint",
        ],
        [
            "effective_configuration",
            "standards_root",
            "actor_usability",
        ],
        ["effective_configuration", "standards_root", "freshness"],
        [
            "effective_configuration",
            "standards_root",
            "projection_position",
        ],
    ] {
        if !json_path_exists(&json, &path) {
            return Err(InstallStepError::StdoutJsonMissingField {
                field_path: path.join("."),
                stdout: run.stdout.clone(),
            });
        }
    }
    serde_json::from_value(json).map_err(|source| InstallStepError::StdoutJsonDecode {
        source,
        stdout: run.stdout.clone(),
    })
}

fn json_path_exists(value: &Value, path: &[&str]) -> bool {
    let mut cursor = value;
    for segment in path {
        let Some(next) = cursor.get(*segment) else {
            return false;
        };
        cursor = next;
    }
    true
}

fn assert_effective_configuration_metadata(
    metadata: &EffectiveConfigurationMetadata,
    family: EffectiveConfigurationSettingFamily,
) -> InstallStepResult<()> {
    if metadata.setting_family != family {
        return Err(
            InstallStepError::UnexpectedEffectiveConfigurationSettingFamily {
                expected: family,
                actual: metadata.setting_family,
            },
        );
    }
    if metadata.source_scope != EffectiveConfigurationSourceScope::Project {
        return Err(
            InstallStepError::UnexpectedEffectiveConfigurationSourceScope {
                actual: metadata.source_scope,
            },
        );
    }
    if metadata.resolution_kind != EffectiveConfigurationResolutionKind::Explicit {
        return Err(
            InstallStepError::UnexpectedEffectiveConfigurationResolutionKind {
                actual: metadata.resolution_kind,
            },
        );
    }
    if metadata.policy_constraint != EffectiveConfigurationPolicyConstraint::None {
        return Err(
            InstallStepError::UnexpectedEffectiveConfigurationPolicyConstraint {
                actual: metadata.policy_constraint,
            },
        );
    }
    if metadata.actor_usability != EffectiveConfigurationActorUsability::Usable {
        return Err(
            InstallStepError::UnexpectedEffectiveConfigurationActorUsability {
                actual: metadata.actor_usability,
            },
        );
    }
    if metadata.freshness != EffectiveConfigurationFreshness::Current {
        return Err(
            InstallStepError::UnexpectedEffectiveConfigurationFreshness {
                actual: metadata.freshness,
            },
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct StandardsInspectReport {
    status: StandardsInspectReportStatus,
    command: StandardsInspectReportCommand,
    repository: String,
    profile: String,
    standards_root: StandardsRoot,
    standards_count: usize,
    first_standard_name: String,
    first_standard_path: String,
    effective_configuration: StandardsInspectEffectiveConfigurationReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StandardsInspectReportStatus {
    Ok,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum StandardsInspectReportCommand {
    #[serde(rename = "standards.inspect")]
    StandardsInspect,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
struct StandardsInspectEffectiveConfigurationReport {
    profile: EffectiveConfigurationMetadata,
    standards_root: EffectiveConfigurationMetadata,
}
