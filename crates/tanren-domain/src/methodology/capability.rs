//! Tool capabilities and per-phase capability scopes.
//!
//! A [`ToolCapability`] is the unit of authorization on the agent tool
//! surface. Each tool method in `app-services::methodology::service`
//! requires a specific capability; the MCP transport consults
//! `TANREN_PHASE_CAPABILITIES` (supplied by the orchestrator at dispatch)
//! to decide which tools are callable in the current phase.
//!
//! Per-phase defaults mirror
//! `docs/architecture/subsystems/tools.md` §4 verbatim.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::phase_id::{KnownPhase, PhaseId};

/// A single authorization scope on the agent tool surface.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ToolCapability {
    // Core task operations (§3.1)
    TaskCreate,
    TaskStart,
    TaskComplete,
    TaskRevise,
    TaskAbandon,
    TaskRead,

    // Findings & rubric (§3.2)
    FindingAdd,
    FindingRead,
    FindingLifecycle,
    RubricRecord,
    ComplianceRecord,

    // Spec frontmatter (§3.3)
    SpecFrontmatter,

    // Demo frontmatter (§3.4)
    DemoFrontmatter,
    DemoResults,

    // Signposts (§3.5)
    SignpostAdd,
    SignpostUpdate,

    // Phase lifecycle (§3.6)
    PhaseOutcome,
    PhaseEscalate,

    // Backlog (§3.7)
    IssueCreate,

    // Standards & adherence (§3.8)
    StandardRead,
    AdherenceRecord,

    // Handle-feedback (§3.6)
    FeedbackReply,

    // Generic checks and durable investigations.
    CheckRecord,
    InvestigationRecord,
}

impl ToolCapability {
    /// Ordered list of all known capabilities.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::TaskCreate,
            Self::TaskStart,
            Self::TaskComplete,
            Self::TaskRevise,
            Self::TaskAbandon,
            Self::TaskRead,
            Self::FindingAdd,
            Self::FindingRead,
            Self::FindingLifecycle,
            Self::RubricRecord,
            Self::ComplianceRecord,
            Self::SpecFrontmatter,
            Self::DemoFrontmatter,
            Self::DemoResults,
            Self::SignpostAdd,
            Self::SignpostUpdate,
            Self::PhaseOutcome,
            Self::PhaseEscalate,
            Self::IssueCreate,
            Self::StandardRead,
            Self::AdherenceRecord,
            Self::FeedbackReply,
            Self::CheckRecord,
            Self::InvestigationRecord,
        ]
    }

    /// Short stable `snake_case` tag. Matches the serde representation.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::TaskCreate => "task.create",
            Self::TaskStart => "task.start",
            Self::TaskComplete => "task.complete",
            Self::TaskRevise => "task.revise",
            Self::TaskAbandon => "task.abandon",
            Self::TaskRead => "task.read",
            Self::FindingAdd => "finding.add",
            Self::FindingRead => "finding.read",
            Self::FindingLifecycle => "finding.lifecycle",
            Self::RubricRecord => "rubric.record",
            Self::ComplianceRecord => "compliance.record",
            Self::SpecFrontmatter => "spec.frontmatter",
            Self::DemoFrontmatter => "demo.frontmatter",
            Self::DemoResults => "demo.results",
            Self::SignpostAdd => "signpost.add",
            Self::SignpostUpdate => "signpost.update",
            Self::PhaseOutcome => "phase.outcome",
            Self::PhaseEscalate => "phase.escalate",
            Self::IssueCreate => "issue.create",
            Self::StandardRead => "standard.read",
            Self::AdherenceRecord => "adherence.record",
            Self::FeedbackReply => "feedback.reply",
            Self::CheckRecord => "check.record",
            Self::InvestigationRecord => "investigation.record",
        }
    }

    /// Parse one capability tag.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "task.create" => Self::TaskCreate,
            "task.start" => Self::TaskStart,
            "task.complete" => Self::TaskComplete,
            "task.revise" => Self::TaskRevise,
            "task.abandon" => Self::TaskAbandon,
            "task.read" => Self::TaskRead,
            "finding.add" => Self::FindingAdd,
            "finding.read" => Self::FindingRead,
            "finding.lifecycle" => Self::FindingLifecycle,
            "rubric.record" => Self::RubricRecord,
            "compliance.record" => Self::ComplianceRecord,
            "spec.frontmatter" => Self::SpecFrontmatter,
            "demo.frontmatter" => Self::DemoFrontmatter,
            "demo.results" => Self::DemoResults,
            "signpost.add" => Self::SignpostAdd,
            "signpost.update" => Self::SignpostUpdate,
            "phase.outcome" => Self::PhaseOutcome,
            "phase.escalate" => Self::PhaseEscalate,
            "issue.create" => Self::IssueCreate,
            "standard.read" => Self::StandardRead,
            "adherence.record" => Self::AdherenceRecord,
            "feedback.reply" => Self::FeedbackReply,
            "check.record" => Self::CheckRecord,
            "investigation.record" => Self::InvestigationRecord,
            _ => return None,
        })
    }
}

impl std::fmt::Display for ToolCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.tag())
    }
}

/// Ordered set of capabilities granted for one phase.
///
/// The set is always materialized as a `BTreeSet<ToolCapability>` so the
/// representation is deterministic for hashing and snapshot tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct CapabilityScope(pub BTreeSet<ToolCapability>);

impl CapabilityScope {
    /// Construct from any iterator of capabilities.
    pub fn from_iter_caps<I: IntoIterator<Item = ToolCapability>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }

    /// True iff the scope grants `cap`.
    #[must_use]
    pub fn allows(&self, cap: ToolCapability) -> bool {
        self.0.contains(&cap)
    }

    /// Empty scope — denies everything.
    #[must_use]
    pub fn empty() -> Self {
        Self(BTreeSet::new())
    }
}

/// Canonical phase/capability mapping row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PhaseCapabilityBinding {
    pub phase: KnownPhase,
    pub capabilities: Vec<ToolCapability>,
}

/// Trait that resolves default capabilities for a phase.
pub trait PhaseCapabilityResolver {
    /// Resolve the default capability scope for one phase.
    fn scope_for_phase(&self, phase: &PhaseId) -> Option<CapabilityScope>;
}

/// Built-in phase capability resolver matching the architecture spec.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultPhaseCapabilityResolver;

impl PhaseCapabilityResolver for DefaultPhaseCapabilityResolver {
    fn scope_for_phase(&self, phase: &PhaseId) -> Option<CapabilityScope> {
        default_scope_for_known_phase(phase.known())
    }
}

/// Phase-name-keyed lookup of the default capability scope.
#[must_use]
pub fn default_scope_for_phase(phase: &PhaseId) -> Option<CapabilityScope> {
    DefaultPhaseCapabilityResolver.scope_for_phase(phase)
}

fn default_scope_for_known_phase(phase: Option<KnownPhase>) -> Option<CapabilityScope> {
    let phase = phase?;
    let caps = default_capabilities_for_known_phase(phase);
    Some(CapabilityScope::from_iter_caps(caps.iter().copied()))
}

fn default_capabilities_for_known_phase(phase: KnownPhase) -> &'static [ToolCapability] {
    use ToolCapability::{
        AdherenceRecord, CheckRecord, ComplianceRecord, DemoFrontmatter, DemoResults,
        FeedbackReply, FindingAdd, FindingLifecycle, FindingRead, InvestigationRecord, IssueCreate,
        PhaseEscalate, PhaseOutcome, RubricRecord, SignpostAdd, SignpostUpdate, SpecFrontmatter,
        StandardRead, TaskAbandon, TaskComplete, TaskCreate, TaskRead, TaskRevise, TaskStart,
    };
    match phase {
        KnownPhase::ShapeSpec => &[
            TaskCreate,
            TaskRevise,
            TaskRead,
            SpecFrontmatter,
            DemoFrontmatter,
            SignpostAdd,
            PhaseOutcome,
        ],
        KnownPhase::DoTask => &[
            TaskStart,
            TaskComplete,
            SignpostAdd,
            SignpostUpdate,
            TaskRead,
            PhaseOutcome,
        ],
        KnownPhase::AuditTask | KnownPhase::AuditSpec => &[
            FindingAdd,
            FindingRead,
            FindingLifecycle,
            CheckRecord,
            RubricRecord,
            ComplianceRecord,
            TaskRead,
            PhaseOutcome,
        ],
        KnownPhase::AdhereTask | KnownPhase::AdhereSpec => &[
            StandardRead,
            AdherenceRecord,
            FindingRead,
            FindingLifecycle,
            CheckRecord,
            TaskRead,
            PhaseOutcome,
        ],
        KnownPhase::SpecGate => &[CheckRecord, FindingRead, TaskRead, PhaseOutcome],
        KnownPhase::RunDemo => &[
            DemoResults,
            FindingAdd,
            FindingRead,
            CheckRecord,
            SignpostAdd,
            TaskRead,
            PhaseOutcome,
        ],
        KnownPhase::WalkSpec => &[TaskCreate, TaskRead, PhaseOutcome],
        KnownPhase::HandleFeedback => &[
            TaskCreate,
            IssueCreate,
            FeedbackReply,
            TaskRead,
            PhaseOutcome,
        ],
        KnownPhase::Investigate => &[
            TaskCreate,
            TaskRevise,
            FindingAdd,
            FindingRead,
            InvestigationRecord,
            PhaseEscalate,
            TaskRead,
            PhaseOutcome,
        ],
        KnownPhase::ResolveBlockers => {
            &[TaskCreate, TaskRevise, TaskAbandon, TaskRead, PhaseOutcome]
        }
    }
}

/// Canonical ordered mapping of built-in phases to default capabilities.
#[must_use]
pub fn default_phase_capability_bindings() -> Vec<PhaseCapabilityBinding> {
    let mut out = Vec::with_capacity(KnownPhase::all().len());
    for phase in KnownPhase::all() {
        let caps = default_capabilities_for_known_phase(*phase);
        out.push(PhaseCapabilityBinding {
            phase: *phase,
            capabilities: caps.to_vec(),
        });
    }
    out
}
