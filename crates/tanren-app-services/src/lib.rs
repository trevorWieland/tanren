//! Shared application service layer for all tanren interfaces.
//!
//! Depends on: `tanren-orchestrator`, `tanren-contract`, `tanren-policy`,
//!             `tanren-store`, `tanren-observability`
//!
//! # Responsibilities
//!
//! - Stable use-case APIs consumed by CLI, API, MCP, and TUI binaries
//! - Input mapping and output shaping (domain types to/from interface types)
//! - No direct transport assumptions (no HTTP, no CLI args, no MCP protocol)
//!
//! # Design Rules
//!
//! - This is the only crate that interface binaries should depend on
//!   (plus `contract` for schema types and runtime/harness crates for wiring)
//! - Forgeclaw integrates with tanren through this layer
