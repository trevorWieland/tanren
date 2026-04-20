//! Methodology service layer — shared between `tanren-cli` and
//! `tanren-mcp`.
//!
//! Per `docs/rewrite/tasks/LANE-0.5-IMPL-BRIEF.md`, this crate owns:
//!
//! - The [`service::MethodologyService`] concrete type — one method
//!   per tool in the catalog.
//! - Capability-scope enforcement ([`capabilities`]).
//! - The pure store → `phase-events.jsonl` projector
//!   ([`phase_events`]).
//! - Three-layer orchestrator-artifact enforcement ([`enforcement`]).
//! - Typed error umbrella ([`errors`]) with its wire-facing
//!   [`errors::ToolError`] shape.
//!
//! Waves 8 and 9 extend this module with the remaining tool methods
//! plus the renderer / installer / format drivers.

pub mod capabilities;
pub mod config;
pub mod enforcement;
pub mod errors;
pub mod formats;
pub mod installer;
mod installer_diff;
pub mod mutation_pipeline;
pub mod phase_events;
pub mod renderer;
pub mod rubric_registry;
pub mod service;
pub mod service_artifacts;
pub mod service_evidence;
pub mod service_ext;
mod service_ext_validation;
mod service_findings;
mod service_idempotency;
mod service_phase;
mod service_projection_reconcile;
mod service_standards;
mod service_task_spec;
mod service_tasks;
pub mod source;
pub mod standards;

pub use capabilities::{enforce, parse_scope_env};
pub use enforcement::{EnforcementGuard, FileSnapshot, UnauthorizedEdit};
pub use errors::{MethodologyError, MethodologyResult, ToolError};
pub use mutation_pipeline::{enter_mutation_session, finalize_mutation_session};
pub use phase_events::{
    PhaseEventAttribution, PhaseEventLine, append_jsonl_encoded_line, append_jsonl_line_atomic,
    jsonl_contains_event_id, line_for_envelope, line_for_envelope_with_attribution,
    project_phase_events, render_jsonl,
};
pub use service::{MethodologyService, ProjectionReconcileReport};

// Re-export the domain-layer capability types so transport crates
// (tanren-cli, tanren-mcp) can depend only on tanren-app-services +
// tanren-contract per the workspace layering rule (CRATE_GUIDE.md §7
// rule 2).
pub use tanren_domain::SpecId;
pub use tanren_domain::methodology::capability::{
    CapabilityScope, ToolCapability, default_scope_for_phase,
};
pub use tanren_domain::methodology::phase_id::{KnownPhase, PhaseId};
pub use tanren_domain::methodology::pillar::{Pillar, builtin_pillars};
pub use tanren_domain::methodology::task::RequiredGuard;

/// Re-exported store-layer replay entry point so transports can
/// drive `tanren replay` / `tanren ingest-phase-events` through a
/// single `app-services` dep.
pub use tanren_store::methodology::{ReplayStats, ingest_phase_events};
