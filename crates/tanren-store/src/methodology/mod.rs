//! Methodology projections and replay.
//!
//! Projections are in-memory folds over the event log. Read paths use
//! indexed event queries (`event_type`, `entity_kind`, and
//! `entity_ref`) plus cursor pagination, then reconstruct typed views by
//! delegating to the pure fold functions in `tanren_domain::methodology`.

mod idempotency;
mod outbox;
pub mod projections;
pub mod replay;
mod replay_line_validation;
mod replay_task_state;
mod spec_lookup_projection;
mod task_list_projection;
mod task_status_lookup;
mod task_status_projection;

pub use idempotency::{InsertMethodologyIdempotencyParams, MethodologyIdempotencyEntry};
pub use outbox::{
    AppendPhaseEventOutboxParams, PhaseEventOutboxCursor, PhaseEventOutboxEntry,
    PhaseEventProjectedOutboxEntry,
};
pub use projections::{
    MethodologyEventFetchError, adherence_findings_for_spec, findings_by_ids, findings_for_spec,
    findings_for_task, load_methodology_events, rubric_for_spec, signposts_for_spec,
    tasks_for_spec,
};
pub use replay::{ReplayError, ReplayStats, ingest_phase_events};
pub use task_list_projection::TaskListProjectionRow;
pub use task_status_projection::TaskStatusProjection;
