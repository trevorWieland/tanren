//! Tanren daemon — control-plane runtime and worker process.
//!
//! This is the main background service that:
//! - Consumes dispatch steps from the job queue
//! - Orchestrates harness and environment lifecycle
//! - Manages concurrent execution across lanes
//! - Handles graceful shutdown and crash recovery
//!
//! Depends on: `tanren-app-services`, `tanren-contract`, and runtime/harness
//! crates for composition wiring.

fn main() {}
