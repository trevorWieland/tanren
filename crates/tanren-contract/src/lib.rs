//! External contract representation and versioning for tanren interfaces.
//!
//! Depends on: `tanren-domain`
//!
//! # Responsibilities
//!
//! - API schema mapping (openapi generation from domain types)
//! - MCP tool schema mapping
//! - CLI command schema mapping
//! - Compatibility and version policy
//!
//! # Design Rules
//!
//! - Serialization and schema only — no orchestration logic
//! - Every interface (CLI/API/MCP/TUI) derives from this contract
//! - Contract changes must be backwards-compatible or explicitly versioned
