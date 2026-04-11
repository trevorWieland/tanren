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

// `entity` is public as an escape hatch: external crates in this
// workspace must not reach into it (linking rule: the store owns all
// SQL and all row shapes), but SeaORM's `DeriveEntityModel` macro
// always emits `pub` items, and the `unreachable_pub` lint requires
// every `pub` item to be reachable from a public path. Exposing the
// module (but not re-exporting anything from it at the crate root)
// keeps the lint satisfied without leaking the entity types into the
// documented API.
mod connection;
mod converters;
#[doc(hidden)]
pub mod entity;
mod errors;
mod event_store;
mod job_queue;
mod job_queue_dequeue;
mod migration;
mod params;
mod state_store;
mod store;

pub use errors::{StoreError, StoreResult};
pub use event_store::EventStore;
pub use job_queue::JobQueue;
pub use params::{
    AckAndEnqueueParams, CreateDispatchParams, DEFAULT_QUERY_LIMIT, DequeueParams, DispatchFilter,
    EnqueueStepParams, EventFilter, NackParams, QueuedStep,
};
pub use state_store::StateStore;
pub use store::Store;
