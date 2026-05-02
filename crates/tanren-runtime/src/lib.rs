//! Runtime substrate contract.
//!
//! Per architecture, agent work executes inside isolated execution targets —
//! containers or remote VMs. This crate defines only the trait surface and
//! shape types every substrate must satisfy. Concrete substrate adapters
//! (docker, Hetzner VM, GCP VM, AWS, ...) are separate crates introduced by
//! the slice that first needs them, so the topology scales without
//! pre-stubbing every cloud provider.
//!
//! Unmanaged local worktree execution is **not** a substrate — the runtime
//! architecture rejects it as a core path.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Identifies a provisioned execution target a [`Substrate`] handed out.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionTargetId(String);

impl ExecutionTargetId {
    /// Wrap a substrate-supplied identifier.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A bounded claim on an execution target. A lease owner is permitted to
/// invoke harnesses or gates inside the target until the lease is released
/// or the substrate revokes it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    /// The target this lease grants access to.
    pub target: ExecutionTargetId,
    /// An opaque token the substrate uses to validate operations.
    pub token: String,
}

/// The substrate trait every runtime adapter implements. F-0001 ships only
/// the trait shape; impls live in separate adapter crates.
#[async_trait::async_trait]
pub trait Substrate: Send + Sync {
    /// Provision (or lease) an execution target and return a [`Lease`] over
    /// it.
    ///
    /// # Errors
    ///
    /// Returns [`SubstrateError::Unavailable`] if no target can be acquired.
    async fn acquire(&self) -> Result<Lease, SubstrateError>;

    /// Release a previously-issued lease.
    ///
    /// # Errors
    ///
    /// Returns [`SubstrateError::Unavailable`] if release fails irrevocably.
    async fn release(&self, lease: Lease) -> Result<(), SubstrateError>;
}

/// Errors raised by substrate adapters.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SubstrateError {
    /// The substrate could not provision or release a target.
    #[error("substrate unavailable: {0}")]
    Unavailable(String),
}
