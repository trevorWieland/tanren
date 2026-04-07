//! Canonical domain model for the tanren orchestration engine.
//!
//! This crate owns all domain semantics and has **no internal workspace dependencies**.
//! Everything else in the workspace depends on `tanren-domain`, never the reverse.
//!
//! # Responsibilities
//!
//! - Domain ID newtypes (dispatch, step, lease, user, team, project)
//! - Lifecycle state machines (dispatch status, step status, lease status)
//! - Commands (create dispatch, enqueue step, cancel, request lease, etc.)
//! - Events (dispatch created, step started, step completed, lease provisioned, etc.)
//! - Typed error taxonomy (policy denied, precondition failed, conflict, not found)
//! - Invariant helpers and state transition validation
//!
//! # Design Rules
//!
//! - No external runtime or storage concerns
//! - No async — pure domain logic only
//! - All types must be `Send + Sync` for safe concurrent use
