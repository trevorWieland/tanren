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

pub use idempotency::{InsertMethodologyIdempotencyParams, MethodologyIdempotencyEntry};
pub use outbox::{AppendPhaseEventOutboxParams, PhaseEventOutboxEntry};
pub use projections::{
    MethodologyEventFetchError, adherence_findings_for_spec, findings_for_spec, findings_for_task,
    load_methodology_events, rubric_for_spec, signposts_for_spec, tasks_for_spec,
};
pub use replay::{ReplayError, ReplayStats, ingest_phase_events};
