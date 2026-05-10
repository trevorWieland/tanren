//! Primary projection-authority assertions consulted before filesystem guards.

use crate::steps::install::context::InstallCommandOutcome;
use tanren_testkit::{
    DriftOutput, DriftPathStatus, InstallCommandKind, InstallSummaryOutput, parse_drift_output,
    parse_install_summary_output,
};

use super::context::InstallContext;
use super::manifest_helpers::RepositoryRelativePath;
use super::{InstallStepError, InstallStepResult};

/// Path-level projection contract: written (replaced) or left alone (preserved).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PathExpectation {
    Preserved,
    Replaced,
}

/// Primary: assert typed command output confirms zero writes.
pub(super) fn assert_typed_zero_writes(
    run: &InstallCommandOutcome,
    kind: InstallCommandKind,
) -> InstallStepResult<()> {
    match kind {
        InstallCommandKind::Drift => {
            if parse_drift_output_or_err(run)?.has_drift() {
                return Err(InstallStepError::ProjectionAuthorityConflict {
                    reason: "drift output reports drift but zero writes were expected".to_owned(),
                });
            }
        }
        InstallCommandKind::Install => {
            let s = parse_install_summary_or_err(run)?;
            if !s.is_zero_write() {
                return Err(InstallStepError::ProjectionAuthorityConflict {
                    reason: format!(
                        "install reports created={} updated={} removed={} restored={} but zero writes expected",
                        s.created_count(),
                        s.updated_count(),
                        s.removed_count(),
                        s.restored_count(),
                    ),
                });
            }
        }
    }
    Ok(())
}

/// Primary for preserved/replaced: verify typed projection output before filesystem.
pub(super) fn assert_path_projection(
    ctx: &InstallContext,
    relative_path: &RepositoryRelativePath,
    expectation: PathExpectation,
) -> InstallStepResult<()> {
    let run = ctx.require_last_run()?;
    let kind = ctx.require_last_command_kind()?;
    let path_str = relative_path.as_str();
    match kind {
        InstallCommandKind::Drift => assert_drift_path_expectation(run, path_str, expectation),
        InstallCommandKind::Install => assert_install_path_expectation(run, path_str, expectation),
    }
}

fn assert_drift_path_expectation(
    run: &InstallCommandOutcome,
    path_str: &str,
    expectation: PathExpectation,
) -> InstallStepResult<()> {
    let output = parse_drift_output_or_err(run)?;
    match expectation {
        PathExpectation::Preserved => {
            let non_clean = output.details().iter().any(|r| {
                r.path() == path_str
                    && r.status() != DriftPathStatus::Clean
                    && r.status() != DriftPathStatus::AcceptedPreserved
            });
            if non_clean {
                return Err(InstallStepError::ProjectionAuthorityConflict {
                    reason: format!(
                        "drift reports non-clean for '{path_str}' but preserved expected"
                    ),
                });
            }
        }
        PathExpectation::Replaced => {
            let changed = output
                .details()
                .iter()
                .any(|r| r.path() == path_str && r.status() == DriftPathStatus::ChangedGenerated);
            if !changed {
                return Err(InstallStepError::ProjectionAuthorityConflict {
                    reason: format!(
                        "drift missing changed-generated for '{path_str}' but replaced expected"
                    ),
                });
            }
        }
    }
    Ok(())
}

fn assert_install_path_expectation(
    run: &InstallCommandOutcome,
    path_str: &str,
    expectation: PathExpectation,
) -> InstallStepResult<()> {
    let in_write = parse_install_summary_or_err(run)?.is_path_in_write_categories(path_str);
    match expectation {
        PathExpectation::Preserved if in_write => {
            Err(InstallStepError::ProjectionAuthorityConflict {
                reason: format!("install reports '{path_str}' written but preserved expected"),
            })
        }
        PathExpectation::Replaced if !in_write => {
            Err(InstallStepError::ProjectionAuthorityConflict {
                reason: format!(
                    "install does not report '{path_str}' written but replaced expected"
                ),
            })
        }
        _ => Ok(()),
    }
}

fn parse_drift_output_or_err(run: &InstallCommandOutcome) -> InstallStepResult<DriftOutput> {
    parse_drift_output(run.stdout()).map_err(|_| InstallStepError::StdoutMissingExpected {
        expected: "valid drift output".to_owned(),
        diagnostic: run.redacted_diagnostic(),
    })
}

fn parse_install_summary_or_err(
    run: &InstallCommandOutcome,
) -> InstallStepResult<InstallSummaryOutput> {
    parse_install_summary_output(run.stdout()).map_err(|_| {
        InstallStepError::StdoutMissingExpected {
            expected: "valid install summary output".to_owned(),
            diagnostic: run.redacted_diagnostic(),
        }
    })
}
