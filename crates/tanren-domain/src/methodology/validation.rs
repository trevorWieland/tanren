//! Shared methodology invariant validators.
//!
//! These checks are intentionally pure and transport-agnostic so both
//! live tool-service mutations and offline replay ingestion can enforce
//! identical invariants.

use crate::{SpecId, TaskId};

use super::phase_id::{KnownPhase, PhaseId};
use super::task::{ExplicitUserDiscardProvenance, TaskAbandonDisposition};

/// Structured field-level validation issue shared by service + replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub field_path: String,
    pub expected: String,
    pub actual: String,
    pub remediation: String,
}

impl ValidationIssue {
    #[must_use]
    pub fn new(
        field_path: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            field_path: field_path.into(),
            expected: expected.into(),
            actual: actual.into(),
            remediation: remediation.into(),
        }
    }
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "validation failed at {}: expected {}, got {}",
            self.field_path, self.expected, self.actual
        )
    }
}

impl std::error::Error for ValidationIssue {}

/// Validate task-abandon semantics (`disposition` / `replacements` /
/// `explicit_user_discard_provenance`) against the active phase.
///
/// # Errors
/// Returns [`ValidationIssue`] when the payload violates methodology
/// invariants.
pub fn validate_task_abandon_semantics(
    phase: &PhaseId,
    disposition: TaskAbandonDisposition,
    replacements: &[TaskId],
    explicit_user_discard_provenance: &Option<ExplicitUserDiscardProvenance>,
) -> Result<(), ValidationIssue> {
    match disposition {
        TaskAbandonDisposition::Replacement => {
            if replacements.is_empty() {
                return Err(ValidationIssue::new(
                    "/replacements",
                    "at least one replacement task id when disposition=replacement",
                    "replacements=[]",
                    "provide replacement task ids, or use disposition=explicit_user_discard with provenance",
                ));
            }
            if explicit_user_discard_provenance.is_some() {
                return Err(ValidationIssue::new(
                    "/explicit_user_discard_provenance",
                    "null when disposition=replacement",
                    "provided",
                    "remove explicit_user_discard_provenance when using replacement disposition",
                ));
            }
        }
        TaskAbandonDisposition::ExplicitUserDiscard => {
            if !replacements.is_empty() {
                return Err(ValidationIssue::new(
                    "/replacements",
                    "empty when disposition=explicit_user_discard",
                    format!("replacements has {} item(s)", replacements.len()),
                    "clear replacements and keep explicit_user_discard_provenance",
                ));
            }
            if !phase.is_known(KnownPhase::ResolveBlockers) {
                return Err(ValidationIssue::new(
                    "/disposition",
                    "explicit_user_discard is only legal in resolve-blockers phase",
                    phase.as_str(),
                    "run explicit user discard through resolve-blockers and pass typed provenance",
                ));
            }
            if explicit_user_discard_provenance.is_none() {
                return Err(ValidationIssue::new(
                    "/explicit_user_discard_provenance",
                    "non-null provenance when disposition=explicit_user_discard",
                    "null",
                    "set explicit_user_discard_provenance.kind=resolve_blockers with a resolution note",
                ));
            }
        }
    }
    Ok(())
}

/// Validate finding line metadata.
///
/// # Errors
/// Returns [`ValidationIssue`] when any entry in `line_numbers` is 0.
pub fn validate_finding_line_numbers(line_numbers: &[u32]) -> Result<(), ValidationIssue> {
    if let Some((idx, _)) = line_numbers.iter().enumerate().find(|(_, v)| **v == 0) {
        return Err(ValidationIssue::new(
            format!("/line_numbers/{idx}"),
            "positive 1-based line number (> 0)",
            "0",
            "use 1-based source line numbers; remove zeros from line_numbers",
        ));
    }
    Ok(())
}

/// Validate that an attached task belongs to the same spec as a finding.
///
/// # Errors
/// Returns [`ValidationIssue`] when `resolved_task_spec_id` does not
/// match `finding_spec_id`.
pub fn validate_finding_attached_task_spec(
    attached_task: TaskId,
    finding_spec_id: SpecId,
    resolved_task_spec_id: SpecId,
) -> Result<(), ValidationIssue> {
    if finding_spec_id != resolved_task_spec_id {
        return Err(ValidationIssue::new(
            "/attached_task",
            format!("task {attached_task} in spec {finding_spec_id}"),
            format!("task {attached_task} belongs to spec {resolved_task_spec_id}"),
            "attach a task from the same spec as the finding, or omit attached_task",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::NonEmptyString;

    use super::*;

    #[test]
    fn replacement_requires_replacements() {
        let err = validate_task_abandon_semantics(
            &PhaseId::try_new("do-task").expect("phase"),
            TaskAbandonDisposition::Replacement,
            &[],
            &None,
        )
        .expect_err("must fail");
        assert_eq!(err.field_path, "/replacements");
    }

    #[test]
    fn explicit_user_discard_requires_resolve_blockers() {
        let err = validate_task_abandon_semantics(
            &PhaseId::try_new("investigate").expect("phase"),
            TaskAbandonDisposition::ExplicitUserDiscard,
            &[],
            &Some(ExplicitUserDiscardProvenance::ResolveBlockers {
                resolution_note: NonEmptyString::try_new("approved").expect("note"),
            }),
        )
        .expect_err("must fail");
        assert_eq!(err.field_path, "/disposition");
    }

    #[test]
    fn zero_line_number_is_rejected_with_indexed_path() {
        let err = validate_finding_line_numbers(&[3, 0, 8]).expect_err("must fail");
        assert_eq!(err.field_path, "/line_numbers/1");
    }

    #[test]
    fn attached_task_spec_mismatch_is_rejected() {
        let err = validate_finding_attached_task_spec(TaskId::new(), SpecId::new(), SpecId::new())
            .expect_err("must fail");
        assert_eq!(err.field_path, "/attached_task");
    }
}
