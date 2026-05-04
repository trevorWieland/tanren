//! Tanren terminal UI.
//!
//! Thin entry point per
//! `profiles/rust-cargo/architecture/thin-binary-crate.md`. All runtime
//! logic — terminal setup/teardown, the screen state machine, ratatui
//! rendering — lives in `tanren-tui-app`; this `main` initializes
//! tracing to a file sink (the TUI owns stdout under raw mode + alternate
//! screen, so a stdout subscriber would corrupt the rendered frame) and
//! hands off. See
//! `docs/architecture/subsystems/observation.md` ("Tracing initialization
//! contract") for the rule, and `xtask check-tracing-init` for enforcement.

use anyhow::{Context, Result};
use tanren_tui_app::{Config, run};

fn main() -> Result<()> {
    let _log_guard =
        tanren_observability::init_to_file(tanren_observability::default_filter(), "tanren-tui")
            .context("install tracing subscriber")?;
    run(Config::from_env())
}
