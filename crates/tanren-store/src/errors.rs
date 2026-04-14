//! Store error type.
//!
//! [`StoreError`] is the single error returned by every `tanren-store`
//! public API. It wraps `SeaORM`'s [`sea_orm::DbErr`] transparently,
//! surfaces conversion failures with a stable context tag, and carries
//! lightweight variants for the domain-adjacent failures callers need
//! to react to (not found, state conflicts, invalid transitions).
//!
//! # Security
//!
//! No variant embeds raw SQL text, query parameters, or connection
//! strings in its [`Display`] impl. [`DbErr`]'s own formatter is the
//! only third-party string we forward, and it is the upstream contract
//! of the driver we've already chosen to trust.
//!
//! [`Display`]: std::fmt::Display
//! [`DbErr`]: sea_orm::DbErr

use sea_orm::DbErr;

/// All errors raised by the store layer.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A `SeaORM` database error — connection, query, or driver-level.
    #[error("database error: {0}")]
    Database(#[from] DbErr),

    /// A migration failed to apply. We keep the `SeaORM` migration error
    /// as a string so callers do not pick up a transitive public
    /// dependency on `sea-orm-migration`.
    #[error("migration error: {0}")]
    Migration(String),

    /// A JSON round-trip between an entity model and a domain type
    /// failed. `context` is a stable `&'static str` identifying the
    /// conversion site (e.g., `events::to_model`); `reason` is the
    /// free-form reason the conversion was rejected. We deliberately
    /// do not embed the payload.
    #[error("conversion error in {context}: {reason}")]
    Conversion {
        /// Stable identifier for the conversion site.
        context: &'static str,
        /// Human-readable reason the conversion failed.
        reason: String,
    },

    /// A `serde_json` (de)serialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// The caller asked for an entity that does not exist. The
    /// `entity` field is an already-rendered identifier (never raw
    /// user input).
    #[error("entity not found: {entity}")]
    NotFound {
        /// Already-rendered identifier describing what was missing.
        entity: String,
    },

    /// A projection row was found in an unexpected state for the
    /// requested transition. Used by `ack`, `ack_and_enqueue`, and
    /// `nack` when the row affected by the update is not exactly 1.
    #[error("invalid state transition on {entity}: {from} -> {to}")]
    InvalidTransition {
        /// The entity whose status we tried to change.
        entity: String,
        /// The status we expected the row to be in.
        from: String,
        /// The status we were attempting to move the row to.
        to: String,
    },

    /// A concurrency conflict — e.g., `ack_and_enqueue` found the
    /// current step already completed by another worker.
    #[error("concurrency conflict: {0}")]
    Conflict(String),
}

/// Convenient alias used throughout the store crate.
pub type StoreResult<T> = Result<T, StoreError>;

impl From<sea_orm::TransactionError<StoreError>> for StoreError {
    fn from(err: sea_orm::TransactionError<StoreError>) -> Self {
        match err {
            sea_orm::TransactionError::Connection(db) => Self::Database(db),
            sea_orm::TransactionError::Transaction(inner) => inner,
        }
    }
}
