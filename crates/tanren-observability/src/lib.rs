//! Shared telemetry primitives for the tanren workspace.
//!
//! Depends on: `tanren-domain`
//!
//! # Responsibilities
//!
//! - Tracing context propagation (correlation IDs, span management)
//! - Metrics registry setup (Prometheus-compatible)
//! - Audit and event correlation helpers
//! - OpenTelemetry integration for traces and metrics export
//!
//! # Design Rules
//!
//! - No crate emits unstructured logs without correlation context
//! - All telemetry uses structured tracing, never `println!` or `eprintln!`
