//! Canonical domain model for the tanren orchestration engine.
//!
//! This crate owns all domain semantics and has **no internal workspace dependencies**.
//! Everything else in the workspace depends on `tanren-domain`, never the reverse.
//!
//! # Responsibilities
//!
//! - Domain ID newtypes (dispatch, step, lease, user, team, org, project)
//! - Lifecycle state machines (dispatch / step / lease) and scheduler ready-state
//! - Actor attribution and typed policy decisions
//! - Commands, events, error taxonomy, and validated value types
//! - Pure guard functions over step projections
//!
//! # Design Rules
//!
//! - No external runtime or storage concerns
//! - No async — pure domain logic only
//! - All types must be `Send + Sync` for safe concurrent use
//! - Validated newtypes enforce construction-time invariants
//! - Domain events never carry secret values; only audit-safe projections

pub mod actor;
pub mod commands;
pub mod entity;
pub mod errors;
pub mod events;
pub mod graph;
pub mod guards;
pub mod ids;
pub mod payloads;
pub mod policy;
pub mod status;
pub mod validated;
pub mod views;

// Re-export the most commonly used types at the crate root for convenience.
pub use self::actor::ActorContext;
pub use self::commands::{
    CancelDispatch, CreateDispatch, EnqueueStep, LeaseCapabilities, ReleaseLease, RequestLease,
    ResourceLimits,
};
pub use self::entity::{EntityKind, EntityRef};
pub use self::errors::{DomainError, ErrorClass, TRANSIENT_BACKOFF, classify_error};
pub use self::events::{
    DomainEvent, EnvelopeDecodeError, EventEnvelope, RawEventEnvelope, SCHEMA_VERSION,
};
pub use self::graph::GraphRevision;
pub use self::guards::{check_execute_guards, check_teardown_guards};
pub use self::ids::{
    ApiKeyId, DispatchId, EventId, LeaseId, OrgId, ProjectId, StepId, TeamId, UserId,
};
pub use self::payloads::{
    ConfigEnv, ConfigKeys, DispatchSnapshot, DryRunPayload, DryRunResult, EnvironmentHandle,
    ExecutePayload, ExecuteResult, Finding, FindingSeverity, ProvisionPayload, ProvisionResult,
    StepPayload, StepResult, TeardownPayload, TeardownResult, TokenUsage,
};
pub use self::policy::{
    PolicyDecisionKind, PolicyDecisionRecord, PolicyOutcome, PolicyResourceRef, PolicyScope,
};
pub use self::status::{
    AuthMode, Cli, DispatchMode, DispatchStatus, Lane, LeaseStatus, Outcome, Phase, StepReadyState,
    StepStatus, StepType, cli_to_lane,
};
pub use self::validated::{FiniteF64, NonEmptyString, TimeoutSecs};
pub use self::views::{DispatchView, EventQueryResult, StepView};
