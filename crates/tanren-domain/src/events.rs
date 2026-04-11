//! Domain events — the canonical history of all state changes.
//!
//! # Envelope and version compatibility
//!
//! Every event is wrapped in an [`EventEnvelope`] carrying a schema
//! version and a typed [`EntityRef`] root. For consumers that need to
//! inspect the schema version before decoding (or preserve unknown
//! variants for later replay), decode into [`RawEventEnvelope`] first
//! and then call [`RawEventEnvelope::try_decode`].

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::actor::ActorContext;
use crate::commands::LeaseCapabilities;
use crate::entity::EntityRef;
use crate::errors::ErrorClass;
use crate::ids::{DispatchId, EventId, LeaseId, StepId};
use crate::payloads::{DispatchSnapshot, StepResult};
use crate::policy::PolicyDecisionRecord;
use crate::status::{DispatchMode, Lane, Outcome, StepType};

/// The current event schema version.
///
/// Bump when any [`DomainEvent`] variant changes shape in a way that
/// older readers cannot silently ignore. Consumers that see a
/// `schema_version` greater than [`SCHEMA_VERSION`] should refuse to
/// decode and upgrade instead.
pub const SCHEMA_VERSION: u32 = 1;

const fn default_schema_version() -> u32 {
    SCHEMA_VERSION
}

/// Fully typed envelope wrapping every domain event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Schema version — defaults to [`SCHEMA_VERSION`] on deserialization
    /// of legacy records that predate the version field.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub event_id: EventId,
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
    /// `entity_ref` from the payload.
    ///
    /// All current [`DomainEvent`] variants are dispatch-scoped, so the
    /// envelope root is always [`EntityRef::Dispatch`] keyed to
    /// `payload.dispatch_id()`. This constructor is the only way to
    /// produce a canonical envelope without risking a mismatch between
    /// `entity_ref` and the payload.
    #[must_use]
    pub fn new(event_id: EventId, timestamp: DateTime<Utc>, payload: DomainEvent) -> Self {
        let entity_ref = EntityRef::Dispatch(payload.dispatch_id());
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
    /// Callers that construct envelopes via direct struct literals (for
    /// tests or migration tooling) can use this to verify the routing
    /// root matches the payload.
    #[must_use]
    pub fn expected_entity_ref(payload: &DomainEvent) -> EntityRef {
        EntityRef::Dispatch(payload.dispatch_id())
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
    ///   This prevents a self-contradictory record where routing code
    ///   trusts `entity_ref` but projections trust `payload.dispatch_id()`.
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

        // Validate that the envelope root matches the payload. The only
        // valid root today is the dispatch the event correlates to.
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
        graph_revision: u32,
        timestamp: DateTime<Utc>,
    },
    DispatchStarted {
        dispatch_id: DispatchId,
        timestamp: DateTime<Utc>,
    },
    DispatchCompleted {
        dispatch_id: DispatchId,
        outcome: Outcome,
        total_duration_secs: f64,
        timestamp: DateTime<Utc>,
    },
    DispatchFailed {
        dispatch_id: DispatchId,
        outcome: Outcome,
        failed_step_id: Option<StepId>,
        failed_step_type: Option<StepType>,
        error: String,
        timestamp: DateTime<Utc>,
    },
    DispatchCancelled {
        dispatch_id: DispatchId,
        /// Actor that initiated the cancellation.
        actor: ActorContext,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },

    // -- Step lifecycle ----------------------------------------------------
    StepEnqueued {
        dispatch_id: DispatchId,
        step_id: StepId,
        step_type: StepType,
        step_sequence: u32,
        lane: Option<Lane>,
        depends_on: Vec<StepId>,
        graph_revision: u32,
        timestamp: DateTime<Utc>,
    },
    StepDequeued {
        dispatch_id: DispatchId,
        step_id: StepId,
        worker_id: String,
        timestamp: DateTime<Utc>,
    },
    StepStarted {
        dispatch_id: DispatchId,
        step_id: StepId,
        worker_id: String,
        step_type: StepType,
        timestamp: DateTime<Utc>,
    },
    StepCompleted {
        dispatch_id: DispatchId,
        step_id: StepId,
        step_type: StepType,
        duration_secs: f64,
        result_payload: Box<StepResult>,
        timestamp: DateTime<Utc>,
    },
    StepFailed {
        dispatch_id: DispatchId,
        step_id: StepId,
        step_type: StepType,
        error: String,
        error_class: ErrorClass,
        retry_count: u32,
        duration_secs: f64,
        timestamp: DateTime<Utc>,
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
        timestamp: DateTime<Utc>,
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
        timestamp: DateTime<Utc>,
    },
    LeaseProvisioned {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        runtime_type: String,
        timestamp: DateTime<Utc>,
    },
    LeaseReady {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        timestamp: DateTime<Utc>,
    },
    LeaseRunning {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        step_id: StepId,
        timestamp: DateTime<Utc>,
    },
    LeaseIdle {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        timestamp: DateTime<Utc>,
    },
    LeaseDraining {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        /// Actor that initiated the drain, if any. `None` when drain is
        /// automatic (idle-TTL expiration, end-of-dispatch cleanup).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caused_by: Option<ActorContext>,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },
    LeaseReleased {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        duration_secs: f64,
        /// Actor that initiated the release, if any. `None` when release
        /// follows automatic drain or post-failure cleanup.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caused_by: Option<ActorContext>,
        timestamp: DateTime<Utc>,
    },
    LeaseFailed {
        lease_id: LeaseId,
        dispatch_id: DispatchId,
        error: String,
        timestamp: DateTime<Utc>,
    },

    // -- Policy ------------------------------------------------------------
    PolicyDecision {
        dispatch_id: DispatchId,
        decision: Box<PolicyDecisionRecord>,
        timestamp: DateTime<Utc>,
    },
}

impl DomainEvent {
    /// Extract the dispatch ID associated with this event.
    #[must_use]
    pub const fn dispatch_id(&self) -> DispatchId {
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
            | Self::PolicyDecision { dispatch_id, .. } => *dispatch_id,
        }
    }

    /// Extract the timestamp of this event.
    #[must_use]
    pub const fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::DispatchCreated { timestamp, .. }
            | Self::DispatchStarted { timestamp, .. }
            | Self::DispatchCompleted { timestamp, .. }
            | Self::DispatchFailed { timestamp, .. }
            | Self::DispatchCancelled { timestamp, .. }
            | Self::StepEnqueued { timestamp, .. }
            | Self::StepDequeued { timestamp, .. }
            | Self::StepStarted { timestamp, .. }
            | Self::StepCompleted { timestamp, .. }
            | Self::StepFailed { timestamp, .. }
            | Self::StepCancelled { timestamp, .. }
            | Self::LeaseRequested { timestamp, .. }
            | Self::LeaseProvisioned { timestamp, .. }
            | Self::LeaseReady { timestamp, .. }
            | Self::LeaseRunning { timestamp, .. }
            | Self::LeaseIdle { timestamp, .. }
            | Self::LeaseDraining { timestamp, .. }
            | Self::LeaseReleased { timestamp, .. }
            | Self::LeaseFailed { timestamp, .. }
            | Self::PolicyDecision { timestamp, .. } => *timestamp,
        }
    }
}
