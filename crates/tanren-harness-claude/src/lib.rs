//! Claude Code harness adapter.
//!
//! Depends on: `tanren-runtime`, `tanren-domain`, `tanren-policy`
//!
//! Implements the harness trait for Claude Code execution — command/prompt
//! preparation, process management, streaming output capture, and telemetry
//! normalization (tokens, duration, retries, errors).
