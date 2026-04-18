//! Capability enforcement on the tool surface.
//!
//! The MCP transport receives its allowed scope via the
//! `TANREN_PHASE_CAPABILITIES` env var (supplied by the orchestrator at
//! dispatch). The CLI transport loads its scope from the session guard
//! state. Both transports share the same `CapabilityScope` type from
//! `tanren_domain::methodology::capability` and run through
//! [`enforce`] below.

use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};

use super::errors::MethodologyError;

/// Return `Ok(())` iff `scope` allows `capability`.
///
/// # Errors
/// Returns [`MethodologyError::CapabilityDenied`] otherwise.
pub fn enforce(
    scope: &CapabilityScope,
    capability: ToolCapability,
    phase: &str,
) -> Result<(), MethodologyError> {
    if scope.allows(capability) {
        Ok(())
    } else {
        Err(MethodologyError::CapabilityDenied {
            capability,
            phase: phase.to_owned(),
        })
    }
}

/// Parse a `TANREN_PHASE_CAPABILITIES` env value into a
/// [`CapabilityScope`]. The value is a comma-separated list of
/// capability tags (e.g. `"task.create,task.read,phase.outcome"`).
///
/// Unknown tags are **ignored** rather than rejected so a stricter
/// orchestrator policy can always narrow the set without coordinating
/// on exact enum tags. Empty input yields an empty scope (denies all).
#[must_use]
pub fn parse_scope_env(value: &str) -> CapabilityScope {
    use ToolCapability::{
        AdherenceRecord, ComplianceRecord, DemoFrontmatter, DemoResults, FeedbackReply, FindingAdd,
        IssueCreate, PhaseEscalate, PhaseOutcome, RubricRecord, SignpostAdd, SignpostUpdate,
        SpecFrontmatter, StandardRead, TaskAbandon, TaskComplete, TaskCreate, TaskRead, TaskRevise,
        TaskStart,
    };
    let known = [
        TaskCreate,
        TaskStart,
        TaskComplete,
        TaskRevise,
        TaskAbandon,
        TaskRead,
        FindingAdd,
        RubricRecord,
        ComplianceRecord,
        SpecFrontmatter,
        DemoFrontmatter,
        DemoResults,
        SignpostAdd,
        SignpostUpdate,
        PhaseOutcome,
        PhaseEscalate,
        IssueCreate,
        StandardRead,
        AdherenceRecord,
        FeedbackReply,
    ];
    let wanted: std::collections::HashSet<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    CapabilityScope::from_iter_caps(known.into_iter().filter(|c| wanted.contains(c.tag())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforce_denies_missing_capability() {
        let scope = CapabilityScope::empty();
        let err = enforce(&scope, ToolCapability::TaskCreate, "do-task").expect_err("must deny");
        assert!(matches!(
            err,
            MethodologyError::CapabilityDenied { phase, .. } if phase == "do-task"
        ));
    }

    #[test]
    fn enforce_allows_granted() {
        let scope = CapabilityScope::from_iter_caps([ToolCapability::TaskStart]);
        enforce(&scope, ToolCapability::TaskStart, "do-task").expect("ok");
    }

    #[test]
    fn parse_scope_env_known_tags() {
        let scope = parse_scope_env("task.create,phase.outcome,unknown.tag");
        assert!(scope.allows(ToolCapability::TaskCreate));
        assert!(scope.allows(ToolCapability::PhaseOutcome));
        assert!(!scope.allows(ToolCapability::TaskStart));
    }

    #[test]
    fn parse_scope_env_empty_is_empty() {
        let scope = parse_scope_env("");
        assert!(!scope.allows(ToolCapability::TaskRead));
    }
}
