//! Tanren terminal UI.
//!
//! Thin entry point per
//! `profiles/rust-cargo/architecture/thin-binary-crate.md`. All runtime
//! logic — terminal setup/teardown, the screen state machine, ratatui
//! rendering — lives in `tanren-tui-app`; this `main` initializes
//! tracing and hands off.

use anyhow::{Context, Result};
use tanren_tui_app::{Config, run};

fn main() -> Result<()> {
    tanren_observability::init(tanren_observability::default_filter())
        .context("install tracing subscriber")?;
    run(Config::from_env())
}
