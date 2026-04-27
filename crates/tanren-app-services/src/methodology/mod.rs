//! Methodology service layer — shared between `tanren-cli` and
//! `tanren-mcp`.
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

pub mod artifact_contract;
mod artifact_projection;
mod artifact_projection_artifacts;
mod artifact_projection_findings;
mod artifact_projection_fold;
mod artifact_projection_helpers;
mod artifact_projection_render;
pub mod assets;
pub mod capabilities;
pub mod config;
pub mod enforcement;
pub mod errors;
pub mod formats;
pub mod installer;
mod installer_binding;
mod installer_diff;
mod installer_walk;
pub mod mutation_pipeline;
pub mod phase_events;
pub mod renderer;
pub mod rubric_registry;
pub mod service;
pub mod service_artifacts;
mod service_check;
pub mod service_evidence;
pub mod service_ext;
mod service_ext_validation;
mod service_findings;
mod service_idempotency;
mod service_investigation;
mod service_phase;
mod service_projection_reconcile;
mod service_spec_shape;
mod service_spec_status;
mod service_spec_status_planner;
mod service_standards;
mod service_task_guard_reset;
mod service_task_spec;
mod service_tasks;
pub mod source;
pub mod standards;

pub use artifact_contract::{
    ARTIFACT_CONTRACT, ArtifactContractEntry, ArtifactProtection, GENERATED_ARTIFACT_MANIFEST_FILE,
    PROJECTION_CHECKPOINT_FILE, append_only_protected_artifacts, generated_manifest_artifacts,
    readonly_artifact_banner, readonly_protected_artifacts,
};
pub use capabilities::{enforce, parse_scope_env, parse_scope_env_for_phase};
pub use enforcement::{EnforcementGuard, FileSnapshot, UnauthorizedEdit};
pub use errors::{MethodologyError, MethodologyResult, ToolError};
pub use mutation_pipeline::{enter_mutation_session, finalize_mutation_session};
pub use phase_events::{
    PhaseEventAttribution, PhaseEventLine, PhaseEventsAppendPolicy, PhaseEventsCompactionReport,
    append_jsonl_encoded_line, append_jsonl_encoded_line_if_missing_event_id,
    append_jsonl_encoded_line_if_missing_event_id_with_policy, append_jsonl_line_atomic,
    compact_jsonl_event_log, jsonl_contains_event_id, line_for_envelope,
    line_for_envelope_with_attribution, project_phase_events, render_jsonl,
};
pub use service::{
    MethodologyRuntimeTuning, MethodologyService, PhaseEventsMaintenanceReport,
    ProjectionReconcileReport,
};

// Re-export the domain-layer capability types so transport crates
// (tanren-cli, tanren-mcp) can depend only on tanren-app-services +
// tanren-contract per the workspace layering rule (CRATE_GUIDE.md §7
// rule 2).
pub use tanren_domain::SpecId;
pub use tanren_domain::methodology::capability::{
    CapabilityScope, PhaseCapabilityBinding, ToolCapability, default_phase_capability_bindings,
    default_scope_for_phase,
};
pub use tanren_domain::methodology::phase_id::{KnownPhase, PhaseId};
pub use tanren_domain::methodology::pillar::{Pillar, builtin_pillars};
pub use tanren_domain::methodology::task::RequiredGuard;
pub use tanren_domain::methodology::tool_catalog::{
    ToolDescriptor, ToolId, all_tool_descriptors, descriptor, descriptor_by_name,
};

/// Re-exported store-layer replay entry point so transports can
/// drive `tanren replay` / `tanren ingest-phase-events` through a
/// single `app-services` dep.
pub use tanren_store::methodology::{ReplayStats, ingest_phase_events};
