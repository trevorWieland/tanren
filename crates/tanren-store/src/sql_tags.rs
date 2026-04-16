//! Canonical SQL-side string tags for domain enums.
//!
//! The store occasionally runs raw SQL (dequeue fast path, cancel
//! batch update) that compares enum columns with string literals.
//! Every such literal lives here so that:
//!
//! 1. Only one file in the crate owns these strings.
//! 2. A single exhaustive guard test maps each constant back to the
//!    domain enum's [`std::fmt::Display`] output, catching drift.
//! 3. The guard uses explicit `match` arms with **no wildcard**, so
//!    adding a new variant to any referenced enum in `tanren-domain`
//!    forces a compile error here instead of silently diverging from
//!    the persistence layer.
//!
//! Modules that emit raw SQL import from this module; they must not
//! redefine the same tags locally.
//!
//! ```text
//! crate::job_queue_dequeue  ──┐
//!                             ├──→ crate::sql_tags
//! crate::state_store_cancel ──┘
//! ```
//!
//! Only tags that appear in raw SQL are exported as `pub(crate)`
//! constants; variants not used by any current raw statement stay
//! inline in the guard test. Adding a new variant to a domain enum
//! still forces a decision here because the match arms are
//! exhaustive.

/// Dispatch/step `status` column — `pending` variant.
pub(crate) const STATUS_PENDING: &str = "pending";
/// Dispatch/step `status` column — `running` variant.
pub(crate) const STATUS_RUNNING: &str = "running";
/// Dispatch/step `status` column — `cancelled` variant.
pub(crate) const STATUS_CANCELLED: &str = "cancelled";

/// Step `ready_state` column — `ready` variant.
pub(crate) const READY_STATE_READY: &str = "ready";

/// Step `step_type` column — `teardown` variant.
pub(crate) const STEP_TYPE_TEARDOWN: &str = "teardown";

#[cfg(test)]
mod tests {
    use super::{
        READY_STATE_READY, STATUS_CANCELLED, STATUS_PENDING, STATUS_RUNNING, STEP_TYPE_TEARDOWN,
    };
    use tanren_domain::{DispatchStatus, Lane, StepReadyState, StepStatus, StepType};

    /// Force compile-time exhaustiveness: adding a new variant to any
    /// of the referenced domain enums causes this test to stop
    /// compiling because the `match` arms have no wildcard.
    ///
    /// Variants that are not currently referenced by raw SQL use the
    /// inline expected tag; any new variant added later has to be
    /// given an arm here and, if it appears in a raw statement, a
    /// `pub(crate) const` above.
    #[test]
    fn sql_tags_match_domain_enums() {
        for status in [
            StepStatus::Pending,
            StepStatus::Running,
            StepStatus::Completed,
            StepStatus::Failed,
            StepStatus::Cancelled,
        ] {
            let expected = match status {
                StepStatus::Pending => STATUS_PENDING,
                StepStatus::Running => STATUS_RUNNING,
                StepStatus::Completed => "completed",
                StepStatus::Failed => "failed",
                StepStatus::Cancelled => STATUS_CANCELLED,
            };
            assert_eq!(
                expected,
                status.to_string(),
                "StepStatus tag drift for {status:?}"
            );
        }

        for status in [
            DispatchStatus::Pending,
            DispatchStatus::Running,
            DispatchStatus::Completed,
            DispatchStatus::Failed,
            DispatchStatus::Cancelled,
        ] {
            let expected = match status {
                DispatchStatus::Pending => STATUS_PENDING,
                DispatchStatus::Running => STATUS_RUNNING,
                DispatchStatus::Completed => "completed",
                DispatchStatus::Failed => "failed",
                DispatchStatus::Cancelled => STATUS_CANCELLED,
            };
            assert_eq!(
                expected,
                status.to_string(),
                "DispatchStatus tag drift for {status:?}"
            );
        }

        for state in [StepReadyState::Blocked, StepReadyState::Ready] {
            let expected = match state {
                StepReadyState::Blocked => "blocked",
                StepReadyState::Ready => READY_STATE_READY,
            };
            assert_eq!(
                expected,
                state.to_string(),
                "StepReadyState tag drift for {state:?}"
            );
        }

        for step_type in [
            StepType::Provision,
            StepType::Execute,
            StepType::Teardown,
            StepType::DryRun,
        ] {
            let expected = match step_type {
                StepType::Provision => "provision",
                StepType::Execute => "execute",
                StepType::Teardown => STEP_TYPE_TEARDOWN,
                StepType::DryRun => "dry_run",
            };
            assert_eq!(
                expected,
                step_type.to_string(),
                "StepType tag drift for {step_type:?}"
            );
        }

        for lane in [Lane::Impl, Lane::Audit, Lane::Gate] {
            let expected = match lane {
                Lane::Impl => "impl",
                Lane::Audit => "audit",
                Lane::Gate => "gate",
            };
            assert_eq!(expected, lane.to_string(), "Lane tag drift for {lane:?}");
        }
    }
}
