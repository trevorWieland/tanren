//! Authorization and governance decisions for the tanren control plane.
//!
//! Depends on: `tanren-domain`
//!
//! # Responsibilities
//!
//! - Identity scopes (user, team, organization)
//! - Budget and quota limits (per-user, per-team, per-org)
//! - Placement approvals and denials (environment selection policy)
//! - Decision reason model for audit trails
//!
//! # Design Rules
//!
//! - Returns typed decisions, never transport-layer errors
//! - Every denied action has an explicit decision record and reason
//! - All policy decisions are evented and auditable
