//! Filter and parameter types used by the store traits.
//!
//! These types intentionally live in `tanren-store` rather than
//! `tanren-domain`: they are persistence concerns (pagination cursors,
//! dequeue knobs, co-transactional event bundles) that the domain
//! layer has no opinion on. Domain types are reused verbatim inside
//! them — the store never invents a duplicate identifier type.
//!
//! # Co-transactionality contract
//!
//! Any param struct whose name ends in `Params` and that contains an
//! [`EventEnvelope`] field promises that the store will append the
//! event inside the same transaction as the projection write it
//! describes. Callers therefore build the envelope once (with the
//! correct `event_id`, `timestamp`, and `entity_ref`) and hand it to
//! the store — the store never mints events on its own.

use chrono::{DateTime, Utc};
use tanren_domain::{
    ActorContext, DispatchId, DispatchMode, DispatchReadScope, DispatchSnapshot, DispatchStatus,
    DispatchSummary, DispatchView, EntityKind, EntityRef, ErrorClass, EventCursor, EventEnvelope,
    GraphRevision, Lane, Outcome, StepId, StepPayload, StepReadyState, StepResult, StepType,
    UserId,
};

// ---------------------------------------------------------------------------
// Query filters
// ---------------------------------------------------------------------------

/// Default page size for paginated queries.
pub const DEFAULT_QUERY_LIMIT: u64 = 100;
/// Maximum page size accepted for dispatch queries.
pub const MAX_DISPATCH_QUERY_LIMIT: u64 = 500;

/// Filter passed to [`EventStore::query_events`](crate::EventStore::query_events).
///
/// Fields use `Option` for "unfiltered on this dimension". `limit`
/// defaults to [`DEFAULT_QUERY_LIMIT`]. Each
/// filter dimension is backed by an index on the `events` table — see
/// `migration::m_0001_init`.
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    /// Restrict to events whose routing root matches this entity.
    pub entity_ref: Option<EntityRef>,
    /// Restrict to events whose routing root is in this set (OR).
    pub entity_refs: Option<Vec<EntityRef>>,
    /// Restrict to events of a given entity kind.
    pub entity_kind: Option<EntityKind>,
    /// Restrict to a specific `DomainEvent` variant tag (`snake_case`).
    pub event_type: Option<String>,
    /// Earliest event timestamp (inclusive).
    pub since: Option<DateTime<Utc>>,
    /// Latest event timestamp (exclusive).
    pub until: Option<DateTime<Utc>>,
    /// Return rows after this cursor key (keyset pagination).
    pub cursor: Option<EventCursor>,
    /// Max rows to return.
    pub limit: u64,
    /// Compute total count as a separate query.
    pub include_total_count: bool,
}

impl EventFilter {
    /// Construct an empty filter with the default limit.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entity_ref: None,
            entity_refs: None,
            entity_kind: None,
            event_type: None,
            since: None,
            until: None,
            cursor: None,
            limit: DEFAULT_QUERY_LIMIT,
            include_total_count: false,
        }
    }
}

/// Filter passed to [`StateStore::query_dispatches`](crate::StateStore::query_dispatches).
///
/// Every optional dimension corresponds to an index on
/// `dispatch_projection`; see `migration::m_0001_init`.
#[derive(Debug, Clone, Default)]
pub struct DispatchFilter {
    /// Restrict to dispatches in this status.
    pub status: Option<DispatchStatus>,
    /// Restrict to dispatches routed to this lane.
    pub lane: Option<Lane>,
    /// Restrict to dispatches for a particular project (exact match).
    pub project: Option<String>,
    /// Restrict to dispatches submitted by a particular user.
    pub user_id: Option<UserId>,
    /// Restrict reads to policy-authorized actor scope.
    pub read_scope: Option<DispatchReadScope>,
    /// Earliest dispatch creation time (inclusive).
    pub since: Option<DateTime<Utc>>,
    /// Latest dispatch creation time (exclusive).
    pub until: Option<DateTime<Utc>>,
    /// Return rows after this cursor key (keyset pagination).
    pub cursor: Option<DispatchCursor>,
    /// Max rows to return.
    pub limit: u64,
}

impl DispatchFilter {
    /// Construct an empty filter with the default limit.
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: None,
            lane: None,
            project: None,
            user_id: None,
            read_scope: None,
            since: None,
            until: None,
            cursor: None,
            limit: DEFAULT_QUERY_LIMIT,
        }
    }
}

/// Cursor key for dispatch list pagination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchCursor {
    /// Primary ordering key.
    pub created_at: DateTime<Utc>,
    /// Tie-breaker key for deterministic ordering.
    pub dispatch_id: DispatchId,
}

/// Paginated dispatch query result.
#[derive(Debug, Clone)]
pub struct DispatchQueryPage {
    /// Current page of dispatches.
    pub dispatches: Vec<DispatchView>,
    /// Cursor for the next page, if more rows are available.
    pub next_cursor: Option<DispatchCursor>,
}

/// Paginated lean dispatch summary query result.
///
/// Returned by
/// [`StateStore::query_dispatch_summaries`](crate::StateStore::query_dispatch_summaries).
/// Unlike [`DispatchQueryPage`], each row carries only the scalar
/// dispatch fields present on the projection table — no JSON decode
/// runs per row.
#[derive(Debug, Clone)]
pub struct DispatchSummaryQueryPage {
    /// Current page of dispatch summaries.
    pub summaries: Vec<DispatchSummary>,
    /// Cursor for the next page, if more rows are available.
    pub next_cursor: Option<DispatchCursor>,
}

// ---------------------------------------------------------------------------
// Queue params
// ---------------------------------------------------------------------------

/// Parameters for [`JobQueue::dequeue`](crate::JobQueue::dequeue).
#[derive(Debug, Clone)]
pub struct DequeueParams {
    /// Opaque identifier for the worker claiming the step. Stored on
    /// the step projection row for crash recovery.
    pub worker_id: String,
    /// Lane to restrict the claim to. `None` means any lane.
    pub lane: Option<Lane>,
    /// Maximum number of steps that may be simultaneously in
    /// `status='running'` for the given lane filter. The dequeue is a
    /// no-op if the running count already meets or exceeds this.
    pub max_concurrent: u64,
}

/// A step successfully claimed from the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedStep {
    /// Step identifier.
    pub step_id: StepId,
    /// Owning dispatch.
    pub dispatch_id: DispatchId,
    /// Declared step type (provision / execute / teardown / `dry_run`).
    pub step_type: StepType,
    /// Monotonic sequence number within the dispatch.
    pub step_sequence: u32,
    /// Lane the step is pinned to, or `None` for free-floating.
    pub lane: Option<Lane>,
    /// Full step payload to hand to the worker.
    pub payload: StepPayload,
}

/// Parameters for [`JobQueue::enqueue_step`](crate::JobQueue::enqueue_step).
///
/// Pairs with a `StepEnqueued` envelope appended co-transactionally by
/// `ack_and_enqueue` and `enqueue_step`.
#[derive(Debug, Clone)]
pub struct EnqueueStepParams {
    /// Owning dispatch.
    pub dispatch_id: DispatchId,
    /// Step identifier.
    pub step_id: StepId,
    /// Declared step type.
    pub step_type: StepType,
    /// Monotonic sequence number within the dispatch.
    pub step_sequence: u32,
    /// Lane the step is pinned to, if any.
    pub lane: Option<Lane>,
    /// Dependency list — other steps whose completion unblocks this one.
    pub depends_on: Vec<StepId>,
    /// Graph revision this step belongs to.
    pub graph_revision: GraphRevision,
    /// Full step payload.
    pub payload: StepPayload,
    /// Initial ready state — `Ready` if `depends_on` is empty, else `Blocked`.
    pub ready_state: StepReadyState,
    /// `StepEnqueued` envelope appended co-transactionally. Callers
    /// build this with the same `dispatch_id` / `step_id`; the store
    /// does not re-derive it.
    pub enqueue_event: EventEnvelope,
}

/// Parameters for [`JobQueue::ack_and_enqueue`](crate::JobQueue::ack_and_enqueue).
///
/// Completes the current step **and** enqueues its successor — plus the
/// two envelopes that describe both transitions — in a single
/// transaction. If `next_step` is `None`, only the completion half runs.
#[derive(Debug, Clone)]
pub struct AckAndEnqueueParams {
    /// Owning dispatch — validated against the completion event payload.
    pub dispatch_id: DispatchId,
    /// Step being completed.
    pub step_id: StepId,
    /// Declared step type — validated against the completion event and
    /// used as a WHERE filter on the projection UPDATE to prevent
    /// `step_type` divergence between the event log and the projection.
    pub step_type: StepType,
    /// Result payload stored on the step projection row.
    pub result: StepResult,
    /// `StepCompleted` envelope appended co-transactionally.
    pub completion_event: EventEnvelope,
    /// Optional successor step — if present, its row is inserted and
    /// its enqueue envelope is appended in the same transaction.
    pub next_step: Option<EnqueueStepParams>,
}

/// Parameters for [`JobQueue::nack`](crate::JobQueue::nack).
#[derive(Debug, Clone)]
pub struct NackParams {
    /// Owning dispatch — validated against the event payload.
    pub dispatch_id: DispatchId,
    /// Step being failed or retried.
    pub step_id: StepId,
    /// Declared step type — validated against the failure event and
    /// used as a WHERE filter on the projection UPDATE to prevent
    /// `step_type` divergence between the event log and the projection.
    pub step_type: StepType,
    /// Human-readable error text stored on the step row.
    pub error: String,
    /// Typed classification (transient / fatal / ambiguous).
    pub error_class: ErrorClass,
    /// If true, the step is reset to `pending` with an incremented
    /// `retry_count`. If false, it is marked `failed`.
    pub retry: bool,
    /// `StepFailed` envelope appended co-transactionally.
    pub failure_event: EventEnvelope,
}

// ---------------------------------------------------------------------------
// Dispatch projection params
// ---------------------------------------------------------------------------

/// Parameters for [`JobQueue::ack`](crate::JobQueue::ack).
///
/// Completes the given step and appends the caller-supplied
/// `StepCompleted` envelope co-transactionally. Replaces the old
/// bare `ack(&StepId, &StepResult)` signature so that every
/// projection-mutating method co-transactionally appends its
/// companion event.
#[derive(Debug, Clone)]
pub struct AckParams {
    /// Owning dispatch — validated against the event payload.
    pub dispatch_id: DispatchId,
    /// Step being completed.
    pub step_id: StepId,
    /// Declared step type — validated against the completion event and
    /// used as a WHERE filter on the projection UPDATE to prevent
    /// `step_type` divergence between the event log and the projection.
    pub step_type: StepType,
    /// Result payload stored on the step projection row.
    pub result: StepResult,
    /// `StepCompleted` envelope appended co-transactionally.
    pub completion_event: EventEnvelope,
}

/// Parameters for
/// [`JobQueue::cancel_pending_steps`](crate::JobQueue::cancel_pending_steps).
///
/// Cancels every pending non-teardown step belonging to a dispatch
/// and appends one `StepCancelled` envelope per cancelled row in
/// the same transaction. The store mints the timestamp internally
/// so the operation identifier is not caller-controlled.
#[derive(Debug, Clone)]
pub struct CancelPendingStepsParams {
    /// Owning dispatch.
    pub dispatch_id: DispatchId,
    /// Actor initiating the cancellation (written to each
    /// `StepCancelled` event).
    pub actor: Option<ActorContext>,
    /// Human-readable reason.
    pub reason: Option<String>,
}

/// Parameters for an atomic dispatch cancellation transaction.
///
/// This operation:
/// 1. validates the dispatch transition to `cancelled`
/// 2. cancels pending non-teardown steps
/// 3. appends per-step `StepCancelled` events
/// 4. updates the dispatch status
/// 5. appends the `DispatchCancelled` event
#[derive(Debug, Clone)]
pub struct CancelDispatchParams {
    /// Owning dispatch.
    pub dispatch_id: DispatchId,
    /// Actor initiating the cancellation.
    pub actor: ActorContext,
    /// Human-readable reason.
    pub reason: Option<String>,
    /// `DispatchCancelled` lifecycle event appended co-transactionally.
    pub status_event: EventEnvelope,
    /// Replay guard key consumed atomically with the cancellation.
    pub replay_guard: ReplayGuard,
}

/// Parameters for
/// [`StateStore::update_dispatch_status`](crate::StateStore::update_dispatch_status).
///
/// Updates the dispatch projection row and appends the caller-
/// supplied lifecycle event co-transactionally. The store validates
/// that the event's `entity_ref` matches `EntityRef::Dispatch(dispatch_id)`
/// before committing.
#[derive(Debug, Clone)]
pub struct UpdateDispatchStatusParams {
    /// Dispatch identifier.
    pub dispatch_id: DispatchId,
    /// New status.
    pub status: DispatchStatus,
    /// Terminal outcome, if any.
    pub outcome: Option<Outcome>,
    /// The lifecycle event to append co-transactionally.
    pub status_event: EventEnvelope,
}

/// Parameters for
/// [`StateStore::create_dispatch_projection`](crate::StateStore::create_dispatch_projection).
///
/// Creates the projection row and appends the companion
/// `DispatchCreated` envelope in one transaction.
#[derive(Debug, Clone)]
pub struct CreateDispatchParams {
    /// Dispatch identifier.
    pub dispatch_id: DispatchId,
    /// Mode (auto / manual).
    pub mode: DispatchMode,
    /// Execution lane.
    pub lane: Lane,
    /// Snapshot of the dispatch configuration at creation time.
    pub dispatch: DispatchSnapshot,
    /// Actor context (org/user/team/project scope).
    pub actor: ActorContext,
    /// Graph revision this dispatch was planned against.
    pub graph_revision: GraphRevision,
    /// Creation timestamp — also used for the projection row's
    /// `created_at` and `updated_at` fields.
    pub created_at: DateTime<Utc>,
    /// `DispatchCreated` envelope appended co-transactionally.
    pub creation_event: EventEnvelope,
}

/// Parameters for a single transaction that:
/// 1. creates a dispatch projection
/// 2. inserts the initial step projection row
/// 3. appends `DispatchCreated` + `StepEnqueued` events.
#[derive(Debug, Clone)]
pub struct CreateDispatchWithInitialStepParams {
    /// Dispatch creation params.
    pub dispatch: CreateDispatchParams,
    /// Initial step params (must be the provision step at sequence 0).
    pub initial_step: EnqueueStepParams,
    /// Replay guard key consumed atomically with the create path.
    pub replay_guard: ReplayGuard,
}

/// Parameters for replay-protected actor-token consumption.
#[derive(Debug, Clone)]
pub struct ConsumeActorTokenJtiParams {
    /// JWT issuer claim.
    pub issuer: String,
    /// JWT audience claim.
    pub audience: String,
    /// JWT ID claim (`jti`).
    pub jti: String,
    /// Issued-at unix timestamp (`iat`).
    pub iat_unix: i64,
    /// Expiry unix timestamp (`exp`).
    pub exp_unix: i64,
    /// Wall-clock consumed-at time.
    pub consumed_at: DateTime<Utc>,
}

/// Replay guard materialized from a verified actor token.
///
/// Mutating store operations consume this key atomically with their
/// dispatch mutation so replay rejection cannot race the write path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayGuard {
    /// JWT issuer claim.
    pub issuer: String,
    /// JWT audience claim.
    pub audience: String,
    /// JWT ID claim (`jti`).
    pub jti: String,
    /// Issued-at unix timestamp (`iat`).
    pub iat_unix: i64,
    /// Expiry unix timestamp (`exp`).
    pub exp_unix: i64,
}

impl ReplayGuard {
    /// Convert this replay guard into insert params with a concrete
    /// `consumed_at` timestamp.
    #[must_use]
    pub fn into_consume_params(self, consumed_at: DateTime<Utc>) -> ConsumeActorTokenJtiParams {
        ConsumeActorTokenJtiParams {
            issuer: self.issuer,
            audience: self.audience,
            jti: self.jti,
            iat_unix: self.iat_unix,
            exp_unix: self.exp_unix,
            consumed_at,
        }
    }
}

/// Parameters for bounded replay-ledger cleanup.
#[derive(Debug, Clone, Copy)]
pub struct PurgeExpiredActorTokenJtisParams {
    /// Remove rows with `exp_unix < expires_before_unix`.
    pub expires_before_unix: i64,
    /// Maximum rows to delete this run.
    pub limit: u64,
}
