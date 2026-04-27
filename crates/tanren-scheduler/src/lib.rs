//! Dispatch graph scheduling with lane and capability awareness.
//!
//! Depends on: `tanren-domain`, `tanren-policy`
//!
//! # Responsibilities
//!
//! - Lane/capability-aware queueing (impl, audit, gate, provision lanes)
//! - Backpressure management (bounded concurrency per lane)
//! - Scheduling decisions based on policy constraints and available capacity
//!
//! # Design Rules
//!
//! - Scheduling is policy-informed but does not own policy decisions
//! - Outputs are dispatch assignments, not execution actions
