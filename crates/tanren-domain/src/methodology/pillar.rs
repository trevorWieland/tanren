//! Audit-rubric pillars.
//!
//! A [`Pillar`] is a scoring dimension. The 13 defaults in [`builtin_pillars`]
//! mirror `docs/architecture/subsystems/audit.md` §3 verbatim. Callers may
//! override any built-in or add their own via `tanren/rubric.yml`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::validated::NonEmptyString;

/// A single scoring pillar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Pillar {
    /// Stable machine id (e.g. `"completeness"`).
    pub id: PillarId,
    /// Human-readable name (e.g. `"Completeness"`).
    pub name: NonEmptyString,
    /// Short description used when auditing a task scope.
    pub task_description: NonEmptyString,
    /// Short description used when auditing a spec scope.
    pub spec_description: NonEmptyString,
    /// Target score (10 for defaults). Scores below target require
    /// at least one supporting finding.
    pub target_score: PillarScore,
    /// Passing threshold. Scores below require a `fix_now` finding.
    pub passing_score: PillarScore,
    /// Which scope(s) this pillar applies to.
    pub applicable_at: ApplicableAt,
}

/// Stable, machine-readable pillar identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct PillarId(String);

impl PillarId {
    /// Construct a [`PillarId`] from any string slice.
    ///
    /// # Errors
    /// Returns a construction error via [`NonEmptyString::try_new`].
    pub fn try_new(value: impl Into<String>) -> Result<Self, crate::errors::DomainError> {
        Ok(Self(NonEmptyString::try_new(value)?.into_inner()))
    }

    /// Borrow the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PillarId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A 1..=10 pillar score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
#[schemars(transparent)]
pub struct PillarScore(u8);

impl PillarScore {
    /// Attempt to construct a score in `1..=10`.
    ///
    /// # Errors
    /// Returns [`crate::errors::DomainError::InvalidValue`] if the value
    /// is outside the inclusive range `1..=10`.
    pub fn try_new(value: u8) -> Result<Self, crate::errors::DomainError> {
        if !(1..=10).contains(&value) {
            return Err(crate::errors::DomainError::InvalidValue {
                field: "pillar_score".into(),
                reason: format!("must be in 1..=10, got {value}"),
            });
        }
        Ok(Self(value))
    }

    /// The raw value.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl std::fmt::Display for PillarScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for PillarScore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = u8::deserialize(deserializer)?;
        Self::try_new(raw).map_err(serde::de::Error::custom)
    }
}

/// Which audit scope(s) a pillar applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicableAt {
    /// Only task-scope audits.
    TaskOnly,
    /// Only spec-scope audits.
    SpecOnly,
    /// Both task and spec.
    Both,
}

impl ApplicableAt {
    /// True if this pillar applies to the given scope.
    #[must_use]
    pub const fn includes(self, scope: PillarScope) -> bool {
        matches!(
            (self, scope),
            (Self::Both, _)
                | (Self::TaskOnly, PillarScope::Task)
                | (Self::SpecOnly, PillarScope::Spec)
        )
    }
}

/// Audit scope, used when filtering applicable pillars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PillarScope {
    Task,
    Spec,
}

/// Static rows for the 13 built-in pillars. Kept adjacent to
/// [`builtin_pillars`] so the authoritative list is obvious and so the
/// function body stays under the 100-line budget.
type BuiltinRow = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    ApplicableAt,
);
const BUILTIN_ROWS: &[BuiltinRow] = &[
    (
        "completeness",
        "Completeness",
        "Acceptance criteria fully met; no silent deferrals.",
        "Spec's acceptance criteria fully met across all implementing tasks.",
        ApplicableAt::Both,
    ),
    (
        "performance",
        "Performance",
        "No gratuitous inefficiency; benchmarks where SLAs apply.",
        "Spec-level latency, memory, and throughput budgets honored.",
        ApplicableAt::Both,
    ),
    (
        "scalability",
        "Scalability",
        "Scales from N=1 to large N; no hard-coded constants breaking at scale.",
        "End-to-end flows scale under target load profiles.",
        ApplicableAt::Both,
    ),
    (
        "strictness",
        "Strictness",
        "Invariants encoded in types; no stringly-typed state; no unwrap/panic in library code.",
        "Cross-cutting invariants compile-time-enforced across the spec surface.",
        ApplicableAt::Both,
    ),
    (
        "security",
        "Security",
        "No secrets in logs; inputs validated at boundaries; authz enforced; secrets wrapped.",
        "Spec-level threat model addressed; no new unreviewed attack surface.",
        ApplicableAt::Both,
    ),
    (
        "stability",
        "Stability",
        "No panics; tests deterministic; races absent; retries/backoff correct.",
        "Integration tests deterministic; no flaky interactions across tasks.",
        ApplicableAt::Both,
    ),
    (
        "maintainability",
        "Maintainability",
        "Module boundaries sensible; names precise; dead code absent.",
        "Spec-level architecture legible; boundaries and ownership clear.",
        ApplicableAt::Both,
    ),
    (
        "extensibility",
        "Extensibility",
        "Pluggable where variation likely; no premature abstraction.",
        "Spec leaves clean extension seams for anticipated follow-up work.",
        ApplicableAt::Both,
    ),
    (
        "elegance",
        "Elegance",
        "Simplest solution solving the real problem; no boilerplate-for-boilerplate.",
        "Overall spec design is the simplest thing that meets the non-negotiables.",
        ApplicableAt::Both,
    ),
    (
        "style",
        "Style",
        "Matches existing code; 2026 best practices; no legacy patterns kept for themselves.",
        "Spec-wide style is internally consistent and idiomatic to the project.",
        ApplicableAt::Both,
    ),
    (
        "relevance",
        "Relevance",
        "All changes related to the task; no unrelated drive-by edits.",
        "Spec-level changes are scoped to the stated acceptance criteria.",
        ApplicableAt::Both,
    ),
    (
        "modularity",
        "Modularity",
        "Boundaries honor the dependency DAG; no cross-cutting leaks.",
        "Spec-level crate/module graph respects architectural layering.",
        ApplicableAt::Both,
    ),
    (
        "documentation_complete",
        "Documentation Complete",
        "Doc comments updated; stale docs pruned; new public APIs documented.",
        "Spec-level documentation matches the shipped behavior.",
        ApplicableAt::Both,
    ),
];

/// The 13 built-in pillars per `docs/architecture/subsystems/audit.md` §3.
#[must_use]
pub fn builtin_pillars() -> Vec<Pillar> {
    let target = PillarScore::try_new(10).expect("built-in target is 10");
    let passing = PillarScore::try_new(7).expect("built-in passing is 7");
    BUILTIN_ROWS
        .iter()
        .map(|(id, name, td, sd, app)| Pillar {
            id: PillarId::try_new(*id).expect("static id is non-empty"),
            name: NonEmptyString::try_new(*name).expect("static name is non-empty"),
            task_description: NonEmptyString::try_new(*td).expect("static desc is non-empty"),
            spec_description: NonEmptyString::try_new(*sd).expect("static desc is non-empty"),
            target_score: target,
            passing_score: passing,
            applicable_at: *app,
        })
        .collect()
}
