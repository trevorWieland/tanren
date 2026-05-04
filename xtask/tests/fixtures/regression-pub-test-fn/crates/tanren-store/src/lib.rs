//! Regression fixture for `xtask check-test-hooks`.
//!
//! Mirrors a workspace crate's `src/` layout and contains exactly one
//! violation: a `pub fn` whose doc-comment mentions "test" but which
//! is not gated on `#[cfg(test)]` or `#[cfg(feature = "test-hooks")]`.
//! `check-test-hooks` must reject this fixture; if it stops doing so
//! the guard has been weakened.

/// Test-only seed helper. Used by integration tests to populate the
/// store with a known fixture row.
pub fn seed_test_data() {}
