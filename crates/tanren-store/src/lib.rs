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
