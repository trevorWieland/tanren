//! Task graph planning and replanning for planner-native orchestration.
//!
//! Depends on: `tanren-domain`
//!
//! # Responsibilities
//!
//! - Issue/task graph planning (decompose work into dependency-aware graphs)
//! - Dependency graph updates (add/remove/reorder tasks based on evidence)
//! - Replanning triggers and outputs (failure, conflict, or policy denial recovery)
//!
//! # Design Rules
//!
//! - Planning produces explicit dispatch graphs with deterministic state transitions
//! - Replanning is evidence-driven, not ad-hoc
//! - Planner outputs are pure data — execution is delegated to scheduler/orchestrator
