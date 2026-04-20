//! Capability enforcement on the tool surface.
//!
//! The MCP transport resolves its allowed scope from signed capability
//! envelope claims and parses those claim values through
//! [`parse_scope_env`]. The CLI transport can still read a
//! `TANREN_PHASE_CAPABILITIES` override for local fallback transport
//! parity. Both transports share the same `CapabilityScope` type from
//! `tanren_domain::methodology::capability` and run through [`enforce`]
//! below.

use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::phase_id::PhaseId;

use super::errors::MethodologyError;

/// Return `Ok(())` iff `scope` allows `capability`.
///
/// # Errors
/// Returns [`MethodologyError::CapabilityDenied`] otherwise.
pub fn enforce(
    scope: &CapabilityScope,
    capability: ToolCapability,
    phase: &PhaseId,
) -> Result<(), MethodologyError> {
    if scope.allows(capability) {
        Ok(())
    } else {
        Err(MethodologyError::CapabilityDenied {
            capability,
            phase: phase.as_str().to_owned(),
        })
    }
}

/// Parse a `TANREN_PHASE_CAPABILITIES` env value into a
/// [`CapabilityScope`]. The value is a comma-separated list of
/// capability tags (e.g. `"task.create,task.read,phase.outcome"`).
///
/// Unknown tags are rejected with a typed validation error.
///
/// # Errors
/// Returns [`MethodologyError::FieldValidation`] for unknown tags.
pub fn parse_scope_env(value: &str) -> Result<CapabilityScope, MethodologyError> {
    let wanted: std::collections::HashSet<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut granted = Vec::with_capacity(wanted.len());
    let mut unknown = Vec::new();
    for tag in wanted {
        match ToolCapability::from_tag(tag) {
            Some(capability) => granted.push(capability),
            None => unknown.push(tag),
        }
    }
    if !unknown.is_empty() {
        unknown.sort_unstable();
        return Err(MethodologyError::FieldValidation {
            field_path: "/TANREN_PHASE_CAPABILITIES".into(),
            expected: "comma-separated known capability tags".into(),
            actual: unknown.join(", "),
            remediation:
                "remove unknown tags or upgrade the orchestrator/capability schema in lock-step"
                    .into(),
        });
    }
    Ok(CapabilityScope::from_iter_caps(granted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforce_denies_missing_capability() {
        let scope = CapabilityScope::empty();
        let phase = PhaseId::try_new("do-task").expect("phase");
        let err = enforce(&scope, ToolCapability::TaskCreate, &phase).expect_err("must deny");
        assert!(matches!(
            err,
            MethodologyError::CapabilityDenied { phase, .. } if phase == "do-task"
        ));
    }

    #[test]
    fn enforce_allows_granted() {
        let scope = CapabilityScope::from_iter_caps([ToolCapability::TaskStart]);
        let phase = PhaseId::try_new("do-task").expect("phase");
        enforce(&scope, ToolCapability::TaskStart, &phase).expect("ok");
    }

    #[test]
    fn parse_scope_env_known_tags() {
        let scope = parse_scope_env("task.create,phase.outcome").expect("scope");
        assert!(scope.allows(ToolCapability::TaskCreate));
        assert!(scope.allows(ToolCapability::PhaseOutcome));
        assert!(!scope.allows(ToolCapability::TaskStart));
    }

    #[test]
    fn parse_scope_env_empty_is_empty() {
        let scope = parse_scope_env("").expect("scope");
        assert!(!scope.allows(ToolCapability::TaskRead));
    }

    #[test]
    fn parse_scope_env_rejects_unknown_tags() {
        let err = parse_scope_env("task.create,unknown.tag").expect_err("must fail");
        assert!(matches!(
            err,
            MethodologyError::FieldValidation { field_path, .. }
                if field_path == "/TANREN_PHASE_CAPABILITIES"
        ));
    }
}
