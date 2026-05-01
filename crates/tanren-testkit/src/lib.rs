//! Shared test utilities for the Tanren BDD harness.
//!
//! This crate is intentionally not pulled into product code. Only the BDD
//! step-definition crate depends on it. Per architecture, test-only support
//! must not leak into runtime crates.

use serde::{Deserialize, Serialize};

/// A fixture seed used to produce deterministic test data. Default seed is
/// `0` so unset fixtures still serialize stably across runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureSeed(u64);

impl FixtureSeed {
    /// Wrap a numeric seed.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The numeric seed value.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}
