//! Control-plane orchestration engine.
//!
//! Depends on: `tanren-policy`, `tanren-store`, `tanren-domain`
//!
//! # Responsibilities
//!
//! - Command intake path (accept dispatch requests from any interface)
//! - Policy/store coordination for dispatch lifecycle
//! - State transition orchestration (drive dispatch graphs through lifecycle)
//! - Guard rule enforcement (concurrency, ordering, terminal state constraints)
//!
//! # Design Rules
//!
//! - Single source of truth for dispatch lifecycle operations
//! - All interfaces (CLI, API, MCP, TUI) call through this layer
//! - No transport-specific logic — that belongs in the binary crates
//! - Generic over store traits — no hardcoded database logic
//!
//! # Terminal-Event Emission Rule
//!
//! - `DispatchCompleted` → only for `Outcome::Success`
//! - `DispatchFailed` → for `Fail | Blocked | Error | Timeout`
//! - `DispatchCancelled` → for user-initiated cancellation

mod dispatch;
pub mod error;

use tanren_policy::PolicyEngine;

pub use error::OrchestratorError;

/// The orchestration engine.
///
/// Generic over `S` which must implement the store traits. Trait bounds
/// are on the impl blocks, not the struct, so construction is possible
/// without the bounds being satisfied (useful for testing and wiring).
#[derive(Debug)]
pub struct Orchestrator<S> {
    store: S,
    policy: PolicyEngine,
}

impl<S> Orchestrator<S> {
    /// Create a new orchestrator with the given store and policy engine.
    pub fn new(store: S, policy: PolicyEngine) -> Self {
        Self { store, policy }
    }

    /// Borrow the underlying store.
    ///
    /// Useful for the composition root and for querying events in tests.
    pub fn store(&self) -> &S {
        &self.store
    }
}
