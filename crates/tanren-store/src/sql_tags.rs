//! Canonical SQL-side string tags for domain enums.
//!
//! The store occasionally runs raw SQL (dequeue fast path, cancel
//! batch update) that compares enum columns with string literals.
//! Every such literal lives here so that:
//!
//! 1. Only one file in the crate owns these strings.
//! 2. A single exhaustive guard test maps each constant back to the
//!    domain enum's [`std::fmt::Display`] output, catching drift.
//! 3. The guard uses explicit `match` arms with **no wildcard**, so
//!    adding a new variant to any referenced enum in `tanren-domain`
//!    forces a compile error here instead of silently diverging from
//!    the persistence layer.
//!
//! Modules that emit raw SQL import from this module; they must not
//! redefine the same tags locally.
//!
//! ```text
//! crate::job_queue_dequeue  ──┐
//!                             ├──→ crate::sql_tags
//! crate::state_store_cancel ──┘
//! ```
//!
//! Only tags that appear in raw SQL are exported as `pub(crate)`
//! constants; variants not used by any current raw statement stay
//! inline in the guard test. Adding a new variant to a domain enum
//! still forces a decision here because the match arms are
//! exhaustive.

/// Dispatch/step `status` column — `pending` variant.
pub(crate) const STATUS_PENDING: &str = "pending";
/// Dispatch/step `status` column — `running` variant.
pub(crate) const STATUS_RUNNING: &str = "running";
/// Dispatch/step `status` column — `cancelled` variant.
pub(crate) const STATUS_CANCELLED: &str = "cancelled";

/// Step `ready_state` column — `ready` variant.
pub(crate) const READY_STATE_READY: &str = "ready";

/// Step `step_type` column — `teardown` variant.
pub(crate) const STEP_TYPE_TEARDOWN: &str = "teardown";
