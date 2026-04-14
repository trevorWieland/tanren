//! `step_projection` table — materialized step state and job queue
//! backing store.
//!
//! Every lifecycle step of every dispatch is one row here. The queue
//! trait (`JobQueue::dequeue`, `ack_and_enqueue`, …) operates on this
//! table directly, so it doubles as both a read projection and a
//! write-heavy work queue. The index on `(status, created_at)` is
//! what makes dequeue an O(1) scan over the pending head.

use sea_orm::entity::prelude::*;

/// Row shape of the `step_projection` table.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "step_projection")]
pub struct Model {
    /// Step identifier — same UUID as the domain [`StepId`].
    ///
    /// [`StepId`]: tanren_domain::StepId
    #[sea_orm(primary_key, auto_increment = false)]
    pub step_id: Uuid,

    /// Owning dispatch.
    pub dispatch_id: Uuid,

    /// Snake-case tag of the [`StepType`](tanren_domain::StepType).
    pub step_type: String,

    /// Monotonic sequence number within the dispatch. Stored as `i32`
    /// for cross-backend compatibility.
    pub step_sequence: i32,

    /// Snake-case tag of the [`Lane`](tanren_domain::Lane), or `NULL`
    /// for free-floating steps.
    pub lane: Option<String>,

    /// Snake-case tag of the [`StepStatus`](tanren_domain::StepStatus).
    pub status: String,

    /// Snake-case tag of the
    /// [`StepReadyState`](tanren_domain::StepReadyState) — distinct
    /// from `status` so the scheduler can look at dependency
    /// readiness without querying the dependency graph.
    pub ready_state: String,

    /// Dependency list — a JSON array of [`StepId`](tanren_domain::StepId)
    /// UUIDs. Persisted as JSON to avoid a second join table for a
    /// rarely-scanned field.
    #[sea_orm(column_type = "JsonBinary")]
    pub depends_on: Json,

    /// Graph revision this step was planned under.
    pub graph_revision: i32,

    /// The worker currently holding the claim, if `status = 'running'`.
    pub worker_id: Option<String>,

    /// Serialized [`StepPayload`](tanren_domain::StepPayload).
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub payload: Option<Json>,

    /// Serialized [`StepResult`](tanren_domain::StepResult), populated
    /// only after the step reaches a terminal state.
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub result: Option<Json>,

    /// Free-form error text from the worker, if `status = 'failed'`.
    pub error: Option<String>,

    /// Number of times this step has been retried.
    pub retry_count: i32,

    /// Worker-reported liveness timestamp. `NULL` while the step is
    /// pending; set to the wall-clock time on dequeue claim; refreshed
    /// by `JobQueue::heartbeat_step` while the worker holds the
    /// claim. `recover_stale_steps` uses this field — **not**
    /// `updated_at` — to decide whether a running step is stale.
    /// This separation keeps liveness signalling independent of
    /// ordinary row writes (ack, nack, etc.).
    pub last_heartbeat_at: Option<DateTimeUtc>,

    /// Wall-clock creation timestamp.
    pub created_at: DateTimeUtc,

    /// Wall-clock last-modified timestamp. Bumped on any write. No
    /// longer used as a liveness proxy — see `last_heartbeat_at`.
    pub updated_at: DateTimeUtc,
}

/// No declared relations — all joins we care about live inside the
/// store's query methods.
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
