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
pub mod phase_events;
pub mod renderer;
pub mod service;
pub mod service_artifacts;
pub mod service_evidence;
pub mod service_ext;
mod service_findings_and_helpers;
pub mod source;
pub mod standards;

pub use capabilities::{enforce, parse_scope_env};
pub use enforcement::{EnforcementGuard, FileSnapshot, UnauthorizedEdit};
pub use errors::{MethodologyError, MethodologyResult, ToolError};
pub use phase_events::{
    PhaseEventLine, append_jsonl_line_atomic, line_for_envelope, project_phase_events, render_jsonl,
};
pub use service::MethodologyService;

// Re-export the domain-layer capability types so transport crates
// (tanren-cli, tanren-mcp) can depend only on tanren-app-services +
// tanren-contract per the workspace layering rule (CRATE_GUIDE.md §7
// rule 2).
pub use tanren_domain::methodology::capability::{
    CapabilityScope, ToolCapability, default_scope_for_phase,
};
pub use tanren_domain::methodology::task::RequiredGuard;

/// Re-exported store-layer replay entry point so transports can
/// drive `tanren replay` / `tanren ingest-phase-events` through a
/// single `app-services` dep.
pub use tanren_store::methodology::{ReplayStats, ingest_phase_events};
