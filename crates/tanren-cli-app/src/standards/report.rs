//! Standards inspect report contract and serialization.

use std::io::Write;
use std::path::Path;

use serde::Serialize;
use tanren_configuration_secrets::{
    EffectiveConfigurationMetadata, EffectiveConfigurationSettingFamily, StandardsRoot,
};

use super::config::{StandardsInspectionTargets, display_repository_argument};
use super::error::{StandardsCommandError, StandardsError};
use super::scan::StandardsScanSummary;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct StandardsInspectSuccessReport {
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

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum StandardsInspectReportStatus {
    Ok,
}

#[derive(Debug, Clone, Copy, Serialize)]
enum StandardsInspectReportCommand {
    #[serde(rename = "standards.inspect")]
    StandardsInspect,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct StandardsInspectEffectiveConfigurationReport {
    profile: EffectiveConfigurationMetadata,
    standards_root: EffectiveConfigurationMetadata,
}

pub(super) fn build_inspect_success_report(
    repository: &Path,
    targets: &StandardsInspectionTargets,
    scan_summary: StandardsScanSummary,
) -> Result<StandardsInspectSuccessReport, StandardsCommandError> {
    let Some(first_standard_name) = scan_summary.first_standard_name else {
        return Err(StandardsCommandError::standards_missing(
            StandardsError::NoStandardsFiles {
                path: targets.standards_root().as_str().to_owned(),
            },
        ));
    };
    let Some(first_standard_path) = scan_summary.first_standard_path else {
        return Err(StandardsCommandError::standards_missing(
            StandardsError::NoStandardsFiles {
                path: targets.standards_root().as_str().to_owned(),
            },
        ));
    };

    Ok(StandardsInspectSuccessReport {
        status: StandardsInspectReportStatus::Ok,
        command: StandardsInspectReportCommand::StandardsInspect,
        repository: display_repository_argument(repository),
        profile: targets.profile().as_str().to_owned(),
        standards_root: targets.standards_root().clone(),
        standards_count: scan_summary.standards_count,
        first_standard_name,
        first_standard_path,
        effective_configuration: StandardsInspectEffectiveConfigurationReport {
            profile: EffectiveConfigurationMetadata::project_explicit(
                EffectiveConfigurationSettingFamily::StandardsProfile,
            ),
            standards_root: EffectiveConfigurationMetadata::project_explicit(
                EffectiveConfigurationSettingFamily::StandardsRoot,
            ),
        },
    })
}

pub(super) fn write_success_report(
    writer: &mut impl Write,
    report: &StandardsInspectSuccessReport,
) -> Result<(), StandardsCommandError> {
    serde_json::to_writer(&mut *writer, report)
        .map_err(StandardsCommandError::report_serialize_failure)?;
    writeln!(writer).map_err(StandardsCommandError::stdout_write_failure)
}
