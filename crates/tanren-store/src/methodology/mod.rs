//! Methodology projections and replay — no new tables.
//!
//! Per Lane 0.5 decision §7, methodology projections are **in-memory
//! folds** over the event log. Queries pull a bounded stream of
//! `DomainEvent::Methodology { event }` rows and reconstruct the
//! requested view by delegating to the pure fold functions in
//! `tanren_domain::methodology`.
//!
//! Scale assumptions: O(events-per-spec) per query. Lane 0.5 specs
//! have hundreds to low thousands of events. Phase 1+ may add `SeaORM`
//! projection tables behind identical function signatures if profiling
//! demands it.

mod append;
pub mod projections;
pub mod replay;

pub use projections::{
    MethodologyEventFetchError, adherence_findings_for_spec, findings_for_spec, findings_for_task,
    load_methodology_events, rubric_for_spec, signposts_for_spec, tasks_for_spec,
};
pub use replay::{ReplayError, ReplayStats, ingest_phase_events};
