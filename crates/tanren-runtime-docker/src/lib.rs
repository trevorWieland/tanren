//! Docker and `DooD` execution runtime.
//!
//! Depends on: `tanren-runtime`, `tanren-domain`, `tanren-policy`
//!
//! Implements the `ExecutionRuntime` trait for local Docker containers
//! and Docker-outside-of-Docker (`DooD`) execution from compose stacks.
//! Handles container spec, mount/network policy, warm pool, and lease lifecycle.
