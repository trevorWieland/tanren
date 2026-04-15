//! Shared application service layer for all tanren interfaces.
//!
//! Depends on: `tanren-orchestrator`, `tanren-contract`, `tanren-store`,
//!             `tanren-domain`
//!
//! # Responsibilities
//!
//! - Stable use-case APIs consumed by CLI, API, MCP, and TUI binaries
//! - Input mapping and output shaping (contract types ↔ domain types)
//! - Error translation from orchestrator/store to wire-safe responses
//! - No direct transport assumptions (no HTTP, no CLI args, no MCP protocol)
//!
//! # Design Rules
//!
//! - This is the primary crate that interface binaries depend on for
//!   business logic (plus `contract` for schema types)
//! - All transport-specific concerns belong in the binary crates
#![deny(clippy::disallowed_methods)]

pub mod auth;
pub mod compose;
mod dispatch_service;
pub mod error;

pub use auth::{
    ActorTokenVerifier, AuthFailureKind, ReplayGuard, RequestContext, TokenVerificationError,
    VerifiedActorToken,
};
pub use dispatch_service::DispatchService;
