//! Control-plane orchestration engine.
//!
//! Depends on: `tanren-planner`, `tanren-scheduler`, `tanren-policy`,
//!             `tanren-store`, `tanren-runtime`, `tanren-domain`
//!
//! # Responsibilities
//!
//! - Command intake path (accept dispatch requests from any interface)
//! - Planner/scheduler/policy/store/runtime coordination
//! - State transition orchestration (drive dispatch graphs through lifecycle)
//! - Guard rule enforcement (concurrency, ordering, terminal state constraints)
//!
//! # Design Rules
//!
//! - Single source of truth for dispatch lifecycle operations
//! - All interfaces (CLI, API, MCP, TUI) call through this layer
//! - No transport-specific logic — that belongs in the binary crates
