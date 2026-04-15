//! Event-sourced persistence layer for the tanren control plane.
//!
//! Depends on: `tanren-domain`
//!
//! # Responsibilities
//!
//! - Event append APIs (append-only canonical event log)
//! - Projection read/write APIs (materialized views for queries)
//! - Migration lifecycle (schema versioning and upgrades)
//! - Transactional guards for race-safe operations
//!
//! # Design Rules
//!
//! - Only this crate owns SQL and query details
//! - Supports `SQLite` (local/dev) and Postgres (team/enterprise)
//! - Write-side uses transactional guarantees
//! - Read-side uses purpose-built indexed projections (no scan-heavy paths)

// `connection` houses `ConnectConfig` (public) alongside internal
// helpers (`connect`, `connect_with_config`).  Making the module
// `pub(crate)` keeps the helpers private while letting `lib.rs`
// re-export `ConnectConfig` by path.
mod connection;
mod converters;
mod db_error_codes;
#[doc(hidden)]
pub(crate) mod entity;
mod errors;
mod event_store;
mod job_queue;
mod job_queue_dequeue;
mod migration;
mod params;
mod state_store;
mod state_store_cancel;
mod store;
mod token_replay_store;

pub use connection::ConnectConfig;
pub use errors::{StoreConflictClass, StoreError, StoreOperation, StoreResult};
pub use event_store::EventStore;
pub use job_queue::JobQueue;
pub use params::{
    AckAndEnqueueParams, AckParams, CancelDispatchParams, CancelPendingStepsParams,
    ConsumeActorTokenJtiParams, CreateDispatchParams, CreateDispatchWithInitialStepParams,
    DEFAULT_QUERY_LIMIT, DequeueParams, DispatchCursor, DispatchFilter, DispatchQueryPage,
    EnqueueStepParams, EventFilter, MAX_DISPATCH_QUERY_LIMIT, NackParams,
    PurgeExpiredActorTokenJtisParams, QueuedStep, ReplayGuard, UpdateDispatchStatusParams,
};
pub use state_store::StateStore;
#[cfg(feature = "test-hooks")]
pub use state_store::dispatch_query_statement_for_backend;
pub use store::Store;
pub use token_replay_store::TokenReplayStore;
