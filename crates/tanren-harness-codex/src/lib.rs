//! Codex harness adapter.
//!
//! Depends on: `tanren-runtime`, `tanren-domain`, `tanren-policy`
//!
//! Implements the harness trait for Codex execution — command/prompt
//! preparation, process management, streaming output capture, and telemetry
//! normalization (tokens, duration, retries, errors).
