//! Runtime trait contracts for harness and environment abstractions.
//!
//! Depends on: `tanren-domain`
//!
//! # Responsibilities
//!
//! - Harness trait (execute agent CLI, stream results, normalize telemetry)
//! - Environment lease trait (provision, run, drain, release, health)
//! - Normalized run result and event models
//!
//! # Design Rules
//!
//! - Traits only — no concrete implementations (those live in `runtime-*` and `harness-*`)
//! - Runtime crates never own policy decisions
//! - All execution results use normalized domain event types
