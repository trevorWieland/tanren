//! Client Integrations subsystem.
//!
//! Owns *inbound* contracts: webhook receivers, subscriptions, idempotent
//! request semantics, rate limits, and backpressure for external systems
//! that call into Tanren.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable idempotency key supplied by a calling client. Two requests with
/// the same key are treated as the same logical operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Wrap a key string.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the key string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Errors raised by client-integration operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ClientIntegrationError {
    /// The request was rate-limited.
    #[error("rate limited")]
    RateLimited,
}
