//! Domain events — the canonical history of all state changes.
//!
//! # Envelope and version compatibility
//!
//! Every event is wrapped in an [`EventEnvelope`] carrying a schema
//! version and a typed [`EntityRef`] root. For consumers that need to
//! inspect the schema version before decoding (or preserve unknown
//! variants for later replay), decode into [`RawEventEnvelope`] first
//! and then call [`RawEventEnvelope::try_decode`].
//!
//! # Single source of truth for timestamps
//!
//! Timestamps live on the [`EventEnvelope`], **not** inside individual
//! [`DomainEvent`] variants. Every event's occurrence time is
//! `envelope.timestamp`. This collapses the former duplication between
//! `envelope.timestamp` and per-variant `timestamp` fields and
//! eliminates a class of "which timestamp do I trust?" projection bugs.
//! Duration fields like `duration_secs` on `StepCompleted` remain on
//! the payload because they're measurements, not occurrence times.
//!
//! # Schema versioning policy
//!
//! [`SCHEMA_VERSION`] is a single monotonically-increasing counter.
//! Consumers MUST reject envelopes whose `schema_version` is greater
//! than their compiled constant and should treat equal or lower
//! versions as decodable (subject to unknown-variant handling).
//!
//! ## Changes that do NOT require a version bump
//!
//! - Adding a new optional field to an existing variant, as long as it
//!   is marked `#[serde(default)]` and `#[serde(skip_serializing_if = ...)]`.
//! - Adding a new event variant (old readers will fail typed decode for
//!   the new variant and can fall back to [`RawEventEnvelope`] — this is
//!   a *runtime* incompatibility, not a schema bump, so new variants
//!   must be introduced behind a feature-flagged roll-out).
//! - Adding a new value to a `#[serde(other)]`-equipped enum (none
//!   today, but the option is reserved for future enums).
//! - Reordering struct fields (the wire format is key/value based).
//!
//! ## Changes that REQUIRE a version bump
//!
//! - Renaming a field on any existing variant.
//! - Changing a field's type (e.g. `String` → `NonEmptyString`).
//! - Removing a variant, even one never emitted in production.
//! - Removing a field, even `#[serde(skip_serializing_if = ...)]`.
//! - Changing a `#[serde(tag)]` or `#[serde(rename)]` on any variant
//!   (this breaks existing on-disk tags).
//! - Changing the discriminant name on `DomainEvent` (currently
//!   `event_type`) or on any child tagged enum.
//! - Changing the meaning of an existing field (semantic break).
//!
//! ## Migration path on a version bump
//!
//! When [`SCHEMA_VERSION`] is bumped:
//!
//! 1. Introduce a migration function in this crate that takes a
//!    `RawEventEnvelope` at the previous version and returns one at
//!    the current version, or an explicit migration error.
//! 2. `RawEventEnvelope::try_decode` must be updated to invoke the
//!    migration before attempting typed decode when the version is
//!    strictly less than [`SCHEMA_VERSION`].
//! 3. Store crates replay through the migration on read so projections
//!    always see events at the current schema version.
//! 4. The previous version's on-disk format must remain readable for
//!    at least one major release after the bump.
//!
//! Until the first bump, [`SCHEMA_VERSION`] stays at `1` and the only
//! compatibility concern is legacy records written before the field
//! existed (handled by the `default_schema_version` serde default).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::actor::ActorContext;
use crate::commands::LeaseCapabilities;
use crate::entity::EntityRef;
use crate::errors::ErrorClass;
use crate::graph::GraphRevision;
use crate::ids::{DispatchId, EventId, LeaseId, StepId};
use crate::payloads::{DispatchSnapshot, StepResult};
use crate::policy::PolicyDecisionRecord;
use crate::status::{DispatchMode, Lane, Outcome, StepType};
use crate::validated::FiniteF64;

/// The current event schema version.
///
/// See the module-level docs for the rubric that determines when this
/// constant must be bumped. Consumers that observe a raw envelope with
/// a version greater than their compiled `SCHEMA_VERSION` must refuse
/// to decode and upgrade the library instead.
pub const SCHEMA_VERSION: u32 = 1;

const fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

/// Fully typed envelope wrapping every domain event.
///
/// `timestamp` is the sole source of occurrence time — no payload
/// variant carries its own timestamp field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Schema version — defaults to [`SCHEMA_VERSION`] on deserialization
    /// of legacy records that predate the version field.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub event_id: EventId,
    /// Occurrence time of the event. This is the single source of
    /// truth; payload variants do not carry their own timestamp.
    pub timestamp: DateTime<Utc>,
    /// Typed reference to the root entity this event is about. For all
    /// current variants this is [`EntityRef::Dispatch`]; future
    /// non-dispatch events can use other variants without changing
    /// envelope shape.
    pub entity_ref: EntityRef,
    pub payload: DomainEvent,
}

impl EventEnvelope {
    /// Construct an envelope with the current [`SCHEMA_VERSION`], deriving
    /// `entity_ref` from the payload's root entity.
    ///
    /// This is the only public constructor that guarantees the envelope
    /// root cannot disagree with the payload. Callers building envelopes
    /// via direct struct literals (tests or migration tooling) should
    /// use [`Self::expected_entity_ref`] to verify consistency.
    #[must_use]
    pub fn new(event_id: EventId, timestamp: DateTime<Utc>, payload: DomainEvent) -> Self {
        let entity_ref = payload.entity_root();
        Self {
            schema_version: SCHEMA_VERSION,
            event_id,
            timestamp,
            entity_ref,
            payload,
        }
    }

    /// Expected `entity_ref` for a given [`DomainEvent`] payload.
    ///
    /// Delegates to [`DomainEvent::entity_root`] so the authoritative
    /// routing rule lives next to the payload variants themselves.
    #[must_use]
    pub fn expected_entity_ref(payload: &DomainEvent) -> EntityRef {
        payload.entity_root()
    }
}

/// A raw envelope whose payload has not yet been decoded into a typed
/// [`DomainEvent`].
///
/// Readers use this as the first decode stage so they can inspect the
/// schema version and preserve unknown variants instead of hard-failing
/// at serde time. Call [`Self::try_decode`] to obtain a typed
/// [`EventEnvelope`] once the version is known-good.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawEventEnvelope {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub event_id: EventId,
    pub timestamp: DateTime<Utc>,
    pub entity_ref: EntityRef,
    /// Opaque payload — decoded lazily by [`Self::try_decode`].
    pub payload: serde_json::Value,
}

impl RawEventEnvelope {
    /// Attempt to decode the raw payload into a typed [`EventEnvelope`].
    ///
    /// # Errors
    /// - [`EnvelopeDecodeError::UnsupportedVersion`] if the stored
    ///   `schema_version` is greater than [`SCHEMA_VERSION`].
    /// - [`EnvelopeDecodeError::UnknownEvent`] if the payload does not
    ///   match any known [`DomainEvent`] variant (preserves the raw
    ///   payload so callers can log or retry).
    /// - [`EnvelopeDecodeError::EntityMismatch`] if the envelope's
    ///   `entity_ref` does not match the root derived from the payload.
    pub fn try_decode(self) -> Result<EventEnvelope, EnvelopeDecodeError> {
        if self.schema_version > SCHEMA_VERSION {
            return Err(EnvelopeDecodeError::UnsupportedVersion {
                version: self.schema_version,
                current: SCHEMA_VERSION,
            });
        }
        let payload: DomainEvent =
            serde_json::from_value(self.payload.clone()).map_err(|source| {
                EnvelopeDecodeError::UnknownEvent {
                    message: source.to_string(),
                    payload: self.payload.clone(),
                }
            })?;

        let expected = EventEnvelope::expected_entity_ref(&payload);
        if self.entity_ref != expected {
            return Err(EnvelopeDecodeError::EntityMismatch {
                envelope: self.entity_ref,
                expected,
            });
        }

        Ok(EventEnvelope {
            schema_version: self.schema_version,
            event_id: self.event_id,
            timestamp: self.timestamp,
            entity_ref: self.entity_ref,
            payload,
        })
    }
}

/// Error returned when decoding a [`RawEventEnvelope`] fails.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EnvelopeDecodeError {
    /// The raw envelope carries a schema version newer than this
    /// library understands.
    #[error("unsupported schema version {version} (current: {current})")]
    UnsupportedVersion { version: u32, current: u32 },

    /// The payload does not match any known [`DomainEvent`] variant.
    #[error("unknown or malformed event variant: {message}")]
    UnknownEvent {
        message: String,
        payload: serde_json::Value,
    },

    /// The envelope's `entity_ref` does not match the root derived from
    /// the decoded payload.
    #[error("entity ref mismatch: envelope={envelope}, expected={expected}")]
    EntityMismatch {
        envelope: EntityRef,
        expected: EntityRef,
    },
}

/// All domain events produced by the orchestration engine.
///
/// No variant carries its own `timestamp` field — the envelope is the
/// single source of truth for occurrence time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum DomainEvent {
    // -- Dispatch lifecycle ------------------------------------------------
    DispatchCreated {
        dispatch_id: DispatchId,
        dispatch: Box<DispatchSnapshot>,
        mode: DispatchMode,
        lane: Lane,
        actor: ActorContext,
        graph_revision: GraphRevision,
    },
    DispatchStarted {
        dispatch_id: DispatchId,
    },
    DispatchCompleted {
        dispatch_id: DispatchId,
        outcome: Outcome,
        total_duration_secs: FiniteF64,
    },
    DispatchFailed {
        dispatch_id: DispatchId,
        outcome: Outcome,
        failed_step_id: Option<StepId>,
        failed_step_type: Option<StepType>,
        error: String,
    },
    DispatchCancelled {
        dispatch_id: DispatchId,
        /// Actor that initiated the cancellation.
        actor: ActorContext,
        reason: Option<String>,
    },

    // -- Step lifecycle ----------------------------------------------------
    StepEnqueued {
        dispatch_id: DispatchId,
        step_id: StepId,
        step_type: StepType,
        step_sequence: u32,
        lane: Option<Lane>,
        depends_on: Vec<StepId>,
        graph_revision: GraphRevision,
    },
    StepDequeued {
        dispatch_id: DispatchId,
        step_id: StepId,
        worker_id: String,
    },
    StepStarted {
        dispatch_id: DispatchId,
        step_id: StepId,
        worker_id: String,
        step_type: StepType,
    },
    StepCompleted {
        dispatch_id: DispatchId,
        step_id: StepId,
        step_type: StepType,
        duration_secs: FiniteF64,
        result_payload: Box<StepResult>,
    },
    StepFailed {
        dispatch_id: DispatchId,
        step_id: StepId,
        step_type: StepType,
        error: String,
        error_class: ErrorClass,
        retry_count: u32,
        duration_secs: FiniteF64,
    },
    StepCancelled {
        dispatch_id: DispatchId,
        step_id: StepId,
        step_type: StepType,
        /// Actor that initiated the cancellation, if any. `None` when
        /// cancellation cascades from a failed dispatch.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caused_by: Option<ActorContext>,
        reason: Option<String>,
    },

    // -- Lease lifecycle ---------------------------------------------------
    //
    // Every lease event carries the dispatch correlation so projections
    // can rebuild lease history alongside dispatch history.
    LeaseRequested {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        step_id: StepId,
        capabilities: Box<LeaseCapabilities>,
    },
    LeaseProvisioned {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        runtime_type: String,
    },
    LeaseReady {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
    },
    LeaseRunning {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        step_id: StepId,
    },
    LeaseIdle {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
    },
    LeaseDraining {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        /// Actor that initiated the drain, if any. `None` when drain is
        /// automatic (idle-TTL expiration, end-of-dispatch cleanup).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caused_by: Option<ActorContext>,
        reason: Option<String>,
    },
    LeaseReleased {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        duration_secs: FiniteF64,
        /// Actor that initiated the release, if any. `None` when release
        /// follows automatic drain or post-failure cleanup.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caused_by: Option<ActorContext>,
    },
    LeaseFailed {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        error: String,
    },

    // -- Policy ------------------------------------------------------------
    PolicyDecision {
        dispatch_id: DispatchId,
        decision: Box<PolicyDecisionRecord>,
    },

    // -- Methodology -------------------------------------------------------
    //
    // Methodology events are the full spec/task lifecycle. They carry no
    // `dispatch_id`; routing is per the inner
    // event's [`crate::methodology::events::MethodologyEvent::entity_root`].
    // Nesting under one outer variant keeps [`SCHEMA_VERSION`] at 1.
    //
    // Struct-variant form (not newtype) so serde's internally-tagged
    // outer discriminant (`event_type = "methodology"`) doesn't collide
    // with the nested `MethodologyEvent`'s own `event_type` discriminant.
    Methodology {
        event: crate::methodology::events::MethodologyEvent,
    },
}

impl DomainEvent {
    /// Extract the dispatch ID associated with this event, if any.
    ///
    /// Methodology events are rooted on spec/task/finding entities
    /// rather than a dispatch, so this accessor returns [`None`] for
    /// them. Callers that need routing metadata should
    /// prefer [`Self::entity_root`] — it's the source of truth for
    /// `EventEnvelope::expected_entity_ref` and handles both families.
    #[must_use]
    pub fn dispatch_id(&self) -> Option<DispatchId> {
        match self {
            Self::DispatchCreated { dispatch_id, .. }
            | Self::DispatchStarted { dispatch_id, .. }
            | Self::DispatchCompleted { dispatch_id, .. }
            | Self::DispatchFailed { dispatch_id, .. }
            | Self::DispatchCancelled { dispatch_id, .. }
            | Self::StepEnqueued { dispatch_id, .. }
            | Self::StepDequeued { dispatch_id, .. }
            | Self::StepStarted { dispatch_id, .. }
            | Self::StepCompleted { dispatch_id, .. }
            | Self::StepFailed { dispatch_id, .. }
            | Self::StepCancelled { dispatch_id, .. }
            | Self::LeaseRequested { dispatch_id, .. }
            | Self::LeaseProvisioned { dispatch_id, .. }
            | Self::LeaseReady { dispatch_id, .. }
            | Self::LeaseRunning { dispatch_id, .. }
            | Self::LeaseIdle { dispatch_id, .. }
            | Self::LeaseDraining { dispatch_id, .. }
            | Self::LeaseReleased { dispatch_id, .. }
            | Self::LeaseFailed { dispatch_id, .. }
            | Self::PolicyDecision { dispatch_id, .. } => Some(*dispatch_id),
            Self::Methodology { .. } => None,
        }
    }

    /// Return the typed root [`EntityRef`] this event correlates to.
    ///
    /// Dispatch/step/lease/policy variants return
    /// [`EntityRef::Dispatch`]; methodology variants delegate to the
    /// inner event's own root (spec / task / finding / signpost /
    /// issue).
    #[must_use]
    pub fn entity_root(&self) -> EntityRef {
        match self {
            Self::DispatchCreated { dispatch_id, .. }
            | Self::DispatchStarted { dispatch_id, .. }
            | Self::DispatchCompleted { dispatch_id, .. }
            | Self::DispatchFailed { dispatch_id, .. }
            | Self::DispatchCancelled { dispatch_id, .. }
            | Self::StepEnqueued { dispatch_id, .. }
            | Self::StepDequeued { dispatch_id, .. }
            | Self::StepStarted { dispatch_id, .. }
            | Self::StepCompleted { dispatch_id, .. }
            | Self::StepFailed { dispatch_id, .. }
            | Self::StepCancelled { dispatch_id, .. }
            | Self::LeaseRequested { dispatch_id, .. }
            | Self::LeaseProvisioned { dispatch_id, .. }
            | Self::LeaseReady { dispatch_id, .. }
            | Self::LeaseRunning { dispatch_id, .. }
            | Self::LeaseIdle { dispatch_id, .. }
            | Self::LeaseDraining { dispatch_id, .. }
            | Self::LeaseReleased { dispatch_id, .. }
            | Self::LeaseFailed { dispatch_id, .. }
            | Self::PolicyDecision { dispatch_id, .. } => EntityRef::Dispatch(*dispatch_id),
            Self::Methodology { event } => event.entity_root(),
        }
    }
}
