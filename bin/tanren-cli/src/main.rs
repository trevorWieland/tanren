//! Tanren scriptable command-line client.
//!
//! Thin entry point per
//! `profiles/rust-cargo/architecture/thin-binary-crate.md`. All runtime
//! logic lives in `tanren-cli-app`; this `main` parses the CLI,
//! initializes tracing, and hands off.

use std::process::ExitCode;

use anyhow::{Context, Result};
use tanren_cli_app::{Config, run};

fn main() -> Result<ExitCode> {
    tanren_observability::init(tanren_observability::default_filter())
        .context("install tracing subscriber")?;
    let config = Config::parse_from_env();
    Ok(run(config))
}
