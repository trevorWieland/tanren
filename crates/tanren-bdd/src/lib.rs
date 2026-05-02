//! BDD step-definition home for Tanren.
//!
//! This is the only crate in the workspace permitted to define `#[test]`
//! items — `xtask check-rust-test-surface` mechanically rejects them
//! anywhere else. F-0001 ships only the registry machinery and a single
//! compile-time smoke check; concrete step definitions and feature files
//! enter with R-0001 onwards.

use cucumber::World as CucumberWorld;
use std::path::PathBuf;
use tanren_testkit::FixtureSeed;

/// Cucumber `World` shared across all Tanren BDD scenarios.
///
/// Slices add per-feature mixins by extending this struct rather than
/// defining their own `World` — keeping the world singular preserves
/// cross-feature step reuse.
#[derive(Debug, Default, CucumberWorld)]
pub struct TanrenWorld {
    /// Deterministic fixture seed. Defaults to `0` so unset fixtures still
    /// serialize stably across runs.
    pub seed: FixtureSeed,
}

/// Run the cucumber harness against the supplied features directory.
///
/// Treats undefined-step and skipped-scenario outcomes as failures so that
/// `just tests` cannot pass with silently-broken behavior proof — a feature
/// file containing an unmatched step definition fails the gate. On failure
/// the process exits with a non-zero status; on success it returns
/// normally.
///
/// In F-0001 the directory is empty, so cucumber reports zero scenarios and
/// the call exits 0 immediately. The harness machinery itself is exercised
/// by the unit tests inside this crate (see `cargo test -p tanren-bdd`).
pub async fn run_features(features_dir: impl Into<PathBuf>) {
    TanrenWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit(features_dir.into())
        .await;
}

#[cfg(test)]
mod tests {
    //! The only `#[test]` items in the workspace. Existence of these proves
    //! the cucumber registry compiles and the [`TanrenWorld`] type is
    //! constructible; correctness of step definitions is proved by the
    //! cucumber scenarios themselves once R-* slices add them.

    use super::TanrenWorld;
    use tanren_testkit::FixtureSeed;

    #[test]
    fn world_default_is_constructible() {
        let world = TanrenWorld::default();
        assert_eq!(world.seed, FixtureSeed::default());
    }

    #[test]
    fn world_seed_round_trips() {
        let world = TanrenWorld {
            seed: FixtureSeed::new(42),
        };
        assert_eq!(world.seed.value(), 42);
    }
}
