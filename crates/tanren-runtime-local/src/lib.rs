//! Local worktree execution runtime.
//!
//! Depends on: `tanren-runtime`, `tanren-domain`, `tanren-policy`
//!
//! Implements the `ExecutionRuntime` trait for local subprocess execution
//! with git worktree isolation, preflight/postflight checks, and process
//! timeout management.
