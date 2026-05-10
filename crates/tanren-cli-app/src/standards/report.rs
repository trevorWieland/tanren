//! Standards inspect report contract and serialization.

use std::io::Write;
use std::path::Path;

use tanren_contract::{
    EffectiveConfigurationMetadataWire, EffectiveConfigurationSettingFamily as WireSettingFamily,
    StandardsInspectEffectiveConfigurationReport, StandardsInspectReport,
    StandardsInspectReportCommand, StandardsInspectReportStatus,
};

use super::config::{StandardsInspectionTargets, display_repository_argument};
use super::error::{StandardsCommandError, StandardsError};
use super::scan::StandardsScanSummary;

pub(super) fn build_inspect_success_report(
    repository: &Path,
    targets: &StandardsInspectionTargets,
    scan_summary: StandardsScanSummary,
) -> Result<StandardsInspectReport, StandardsCommandError> {
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

    Ok(StandardsInspectReport {
        status: StandardsInspectReportStatus::Ok,
        command: StandardsInspectReportCommand::StandardsInspect,
        repository: display_repository_argument(repository),
        profile: targets.profile().as_str().to_owned(),
        standards_root: targets.standards_root().as_str().to_owned(),
        standards_count: scan_summary.standards_count,
        first_standard_name,
        first_standard_path,
        effective_configuration: StandardsInspectEffectiveConfigurationReport {
            profile: EffectiveConfigurationMetadataWire::project_explicit(
                WireSettingFamily::StandardsProfile,
            ),
            standards_root: EffectiveConfigurationMetadataWire::project_explicit(
                WireSettingFamily::StandardsRoot,
            ),
        },
    })
}

pub(super) fn write_success_report(
    writer: &mut impl Write,
    report: &StandardsInspectReport,
) -> Result<(), StandardsCommandError> {
    serde_json::to_writer(&mut *writer, report)
        .map_err(StandardsCommandError::report_serialize_failure)?;
    writeln!(writer).map_err(StandardsCommandError::stdout_write_failure)
}
