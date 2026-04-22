//! Typed phase identifiers.
//!
//! Replaces stringly-typed phase checks in policy-critical paths.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::validated::NonEmptyString;

/// Canonical phase identifier.
///
/// Accepts known built-in phases and non-empty custom phase names.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct PhaseId(NonEmptyString);

impl PhaseId {
    /// Construct from any non-empty phase label.
    ///
    /// # Errors
    /// Returns [`DomainError::InvalidValue`] on empty values.
    pub fn try_new(value: impl Into<String>) -> Result<Self, DomainError> {
        let raw = value.into();
        let inner =
            NonEmptyString::try_new(raw.clone()).map_err(|e| DomainError::InvalidValue {
                field: "phase".into(),
                reason: e.to_string(),
            })?;
        Ok(Self(inner))
    }

    /// Borrow as `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Resolve to a known built-in phase when possible.
    #[must_use]
    pub fn known(&self) -> Option<KnownPhase> {
        KnownPhase::from_tag(self.as_str())
    }

    /// True when this phase matches the given known phase.
    #[must_use]
    pub fn is_known(&self, expected: KnownPhase) -> bool {
        self.known().is_some_and(|v| v == expected)
    }
}

impl std::fmt::Display for PhaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for PhaseId {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value.to_owned())
    }
}

impl TryFrom<String> for PhaseId {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

/// Known built-in phases.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum KnownPhase {
    ShapeSpec,
    DoTask,
    AuditTask,
    AdhereTask,
    RunDemo,
    AuditSpec,
    AdhereSpec,
    WalkSpec,
    HandleFeedback,
    Investigate,
    ResolveBlockers,
    TriageAudits,
    SyncRoadmap,
    DiscoverStandards,
    IndexStandards,
    InjectStandards,
    PlanProduct,
}

impl KnownPhase {
    /// Ordered list of all built-in phases.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::ShapeSpec,
            Self::DoTask,
            Self::AuditTask,
            Self::AdhereTask,
            Self::RunDemo,
            Self::AuditSpec,
            Self::AdhereSpec,
            Self::WalkSpec,
            Self::HandleFeedback,
            Self::Investigate,
            Self::ResolveBlockers,
            Self::TriageAudits,
            Self::SyncRoadmap,
            Self::DiscoverStandards,
            Self::IndexStandards,
            Self::InjectStandards,
            Self::PlanProduct,
        ]
    }

    /// Stable kebab-case tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::ShapeSpec => "shape-spec",
            Self::DoTask => "do-task",
            Self::AuditTask => "audit-task",
            Self::AdhereTask => "adhere-task",
            Self::RunDemo => "run-demo",
            Self::AuditSpec => "audit-spec",
            Self::AdhereSpec => "adhere-spec",
            Self::WalkSpec => "walk-spec",
            Self::HandleFeedback => "handle-feedback",
            Self::Investigate => "investigate",
            Self::ResolveBlockers => "resolve-blockers",
            Self::TriageAudits => "triage-audits",
            Self::SyncRoadmap => "sync-roadmap",
            Self::DiscoverStandards => "discover-standards",
            Self::IndexStandards => "index-standards",
            Self::InjectStandards => "inject-standards",
            Self::PlanProduct => "plan-product",
        }
    }

    /// Parse from kebab-case tag.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "shape-spec" => Some(Self::ShapeSpec),
            "do-task" => Some(Self::DoTask),
            "audit-task" => Some(Self::AuditTask),
            "adhere-task" => Some(Self::AdhereTask),
            "run-demo" => Some(Self::RunDemo),
            "audit-spec" => Some(Self::AuditSpec),
            "adhere-spec" => Some(Self::AdhereSpec),
            "walk-spec" => Some(Self::WalkSpec),
            "handle-feedback" => Some(Self::HandleFeedback),
            "investigate" => Some(Self::Investigate),
            "resolve-blockers" => Some(Self::ResolveBlockers),
            "triage-audits" => Some(Self::TriageAudits),
            "sync-roadmap" => Some(Self::SyncRoadmap),
            "discover-standards" => Some(Self::DiscoverStandards),
            "index-standards" => Some(Self::IndexStandards),
            "inject-standards" => Some(Self::InjectStandards),
            "plan-product" => Some(Self::PlanProduct),
            _ => None,
        }
    }
}

impl std::fmt::Display for KnownPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.tag())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_id_rejects_empty() {
        assert!(PhaseId::try_new("").is_err());
        assert!(PhaseId::try_new("   ").is_err());
    }

    #[test]
    fn known_phase_roundtrip() {
        for k in KnownPhase::all() {
            let phase = PhaseId::try_new(k.tag()).expect("valid");
            assert_eq!(phase.known(), Some(*k));
            let json = serde_json::to_string(&phase).expect("serialize");
            let back: PhaseId = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back.known(), Some(*k));
        }
    }
}
