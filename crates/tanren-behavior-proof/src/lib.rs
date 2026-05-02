//! Behavior Proof subsystem.
//!
//! Owns the link between accepted behaviors (`B-XXXX` ids) and the BDD
//! scenarios that prove them, plus mutation-testing signal interpretation.
//! The cucumber harness itself lives in `tanren-bdd`; this crate owns the
//! *meaning* of proof, not the runner.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable behavior id of the form `B-XXXX`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BehaviorId(String);

impl BehaviorId {
    /// Wrap a behavior id string. Format validation lands with the slice
    /// that introduces the inventory check.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the id string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The two witness kinds every asserted behavior must carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Witness {
    /// A scenario asserting the behavior holds.
    Positive,
    /// A scenario asserting absence-of-bug — the negative space.
    Falsification,
}

/// Errors raised by behavior-proof operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BehaviorProofError {
    /// A behavior is missing one of its required witness kinds.
    #[error("missing witness {witness:?} for behavior {behavior:?}")]
    MissingWitness {
        /// The behavior whose witness is missing.
        behavior: BehaviorId,
        /// The witness kind that should be present.
        witness: Witness,
    },
}
