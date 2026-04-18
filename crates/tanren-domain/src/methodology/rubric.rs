//! Rubric scoring with construction-time invariants.
//!
//! A [`RubricScore`] records one pillar's score for one audit, along
//! with the findings that justify the gap between score and target.
//! Two invariants are enforced at the type boundary:
//!
//! 1. If `score < target`, `supporting_finding_ids` must be non-empty.
//! 2. If `score < passing`, at least one supporting finding must have
//!    severity `fix_now`. The enforcement of (2) happens in
//!    `app-services::methodology::rubric` where the severity is
//!    resolved from ids — this module enforces (1) directly and holds
//!    the pillar-linkage invariant.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::errors::DomainError;
use crate::ids::FindingId;
use crate::validated::NonEmptyString;

use super::pillar::{PillarId, PillarScore};

/// One scored pillar on one audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RubricScore {
    pub pillar: PillarId,
    pub score: PillarScore,
    pub target: PillarScore,
    pub passing: PillarScore,
    pub rationale: NonEmptyString,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supporting_finding_ids: Vec<FindingId>,
}

impl RubricScore {
    /// Construct a score, enforcing the linkage invariant.
    ///
    /// # Errors
    /// - [`DomainError::InvalidValue`] if `target < passing`.
    /// - [`DomainError::PreconditionFailed`] if `score < target` and
    ///   `supporting_finding_ids` is empty.
    pub fn try_new(
        pillar: PillarId,
        score: PillarScore,
        target: PillarScore,
        passing: PillarScore,
        rationale: NonEmptyString,
        supporting_finding_ids: Vec<FindingId>,
    ) -> Result<Self, DomainError> {
        if target < passing {
            return Err(DomainError::InvalidValue {
                field: "rubric_score.target".into(),
                reason: format!(
                    "target ({}) must be >= passing ({})",
                    target.get(),
                    passing.get(),
                ),
            });
        }
        if score < target && supporting_finding_ids.is_empty() {
            return Err(DomainError::PreconditionFailed {
                reason: format!(
                    "pillar {pillar}: score {} < target {} requires at least one supporting finding",
                    score.get(),
                    target.get(),
                ),
            });
        }
        Ok(Self {
            pillar,
            score,
            target,
            passing,
            rationale,
            supporting_finding_ids,
        })
    }

    /// True if this score meets the passing threshold.
    #[must_use]
    pub fn is_passing(&self) -> bool {
        self.score >= self.passing
    }

    /// True if this score meets the target (no supporting findings required).
    #[must_use]
    pub fn is_at_target(&self) -> bool {
        self.score >= self.target
    }
}

impl<'de> Deserialize<'de> for RubricScore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            pillar: PillarId,
            score: PillarScore,
            target: PillarScore,
            passing: PillarScore,
            rationale: NonEmptyString,
            #[serde(default)]
            supporting_finding_ids: Vec<FindingId>,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::try_new(
            raw.pillar,
            raw.score,
            raw.target,
            raw.passing,
            raw.rationale,
            raw.supporting_finding_ids,
        )
        .map_err(serde::de::Error::custom)
    }
}

/// Compliance status for a named non-negotiable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceStatus {
    Pass,
    Fail,
}

/// Record of one non-negotiable check on an audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NonNegotiableCompliance {
    pub name: NonEmptyString,
    pub status: ComplianceStatus,
    pub rationale: NonEmptyString,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid() -> PillarId {
        PillarId::try_new("security").expect("valid pillar id")
    }

    fn score(n: u8) -> PillarScore {
        PillarScore::try_new(n).expect("valid score")
    }

    fn rationale() -> NonEmptyString {
        NonEmptyString::try_new("rationale text").expect("non-empty")
    }

    #[test]
    fn at_target_no_findings_required() {
        let rs = RubricScore::try_new(pid(), score(10), score(10), score(7), rationale(), vec![])
            .expect("ok");
        assert!(rs.is_at_target());
        assert!(rs.is_passing());
    }

    #[test]
    fn below_target_requires_finding() {
        let err = RubricScore::try_new(pid(), score(8), score(10), score(7), rationale(), vec![])
            .expect_err("below-target with no findings must be rejected");
        assert!(matches!(err, DomainError::PreconditionFailed { .. }));
    }

    #[test]
    fn below_target_with_finding_ok() {
        let rs = RubricScore::try_new(
            pid(),
            score(8),
            score(10),
            score(7),
            rationale(),
            vec![FindingId::new()],
        )
        .expect("ok");
        assert!(rs.is_passing());
        assert!(!rs.is_at_target());
    }

    #[test]
    fn target_below_passing_rejected() {
        let err = RubricScore::try_new(pid(), score(10), score(5), score(7), rationale(), vec![])
            .expect_err("target < passing must be rejected");
        assert!(matches!(err, DomainError::InvalidValue { .. }));
    }

    #[test]
    fn deserialize_runs_invariant() {
        let bad = serde_json::json!({
            "pillar": "security",
            "score": 5,
            "target": 10,
            "passing": 7,
            "rationale": "no findings here",
            "supporting_finding_ids": []
        });
        let err = serde_json::from_value::<RubricScore>(bad)
            .expect_err("below-target with empty findings must fail to deserialize");
        let msg = err.to_string();
        assert!(
            msg.contains("requires at least one supporting finding"),
            "unexpected error message: {msg}"
        );
    }

    #[test]
    fn deserialize_roundtrip() {
        let fid = FindingId::new();
        let rs = RubricScore::try_new(pid(), score(6), score(10), score(7), rationale(), vec![fid])
            .expect("ok");
        let json = serde_json::to_string(&rs).expect("serialize");
        let back: RubricScore = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(rs, back);
    }
}
