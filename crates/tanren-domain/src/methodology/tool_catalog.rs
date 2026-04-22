//! Typed methodology tool catalog.
//!
//! This is the single source of truth for methodology tool identity,
//! capability mapping, CLI routing, and mutation classification.

use super::capability::ToolCapability;

/// Stable identifier for one methodology tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolId {
    CreateTask,
    StartTask,
    CompleteTask,
    MarkTaskGuardSatisfied,
    ReviseTask,
    AbandonTask,
    ListTasks,
    AddFinding,
    RecordRubricScore,
    RecordNonNegotiableCompliance,
    SetSpecTitle,
    SetSpecProblemStatement,
    SetSpecMotivations,
    SetSpecExpectations,
    SetSpecPlannedBehaviors,
    SetSpecImplementationPlan,
    SetSpecNonNegotiables,
    AddSpecAcceptanceCriterion,
    SetSpecDemoEnvironment,
    SetSpecDependencies,
    SetSpecBaseBranch,
    SetSpecRelevanceContext,
    AddDemoStep,
    MarkDemoStepSkip,
    AppendDemoResult,
    AddSignpost,
    UpdateSignpostStatus,
    ReportPhaseOutcome,
    EscalateToBlocker,
    PostReplyDirective,
    CreateIssue,
    ListRelevantStandards,
    RecordAdherenceFinding,
}

impl ToolId {
    /// Ordered list of all methodology tools.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::CreateTask,
            Self::StartTask,
            Self::CompleteTask,
            Self::MarkTaskGuardSatisfied,
            Self::ReviseTask,
            Self::AbandonTask,
            Self::ListTasks,
            Self::AddFinding,
            Self::RecordRubricScore,
            Self::RecordNonNegotiableCompliance,
            Self::SetSpecTitle,
            Self::SetSpecProblemStatement,
            Self::SetSpecMotivations,
            Self::SetSpecExpectations,
            Self::SetSpecPlannedBehaviors,
            Self::SetSpecImplementationPlan,
            Self::SetSpecNonNegotiables,
            Self::AddSpecAcceptanceCriterion,
            Self::SetSpecDemoEnvironment,
            Self::SetSpecDependencies,
            Self::SetSpecBaseBranch,
            Self::SetSpecRelevanceContext,
            Self::AddDemoStep,
            Self::MarkDemoStepSkip,
            Self::AppendDemoResult,
            Self::AddSignpost,
            Self::UpdateSignpostStatus,
            Self::ReportPhaseOutcome,
            Self::EscalateToBlocker,
            Self::PostReplyDirective,
            Self::CreateIssue,
            Self::ListRelevantStandards,
            Self::RecordAdherenceFinding,
        ]
    }

    /// Stable tool name used by MCP and CLI JSON contracts.
    #[must_use]
    pub const fn name(self) -> &'static str {
        descriptor(self).name
    }
}

/// Metadata for one methodology tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolDescriptor {
    pub id: ToolId,
    pub name: &'static str,
    pub capability: ToolCapability,
    pub cli_noun: &'static str,
    pub cli_verb: &'static str,
    pub mutation: bool,
}

const DESCRIPTORS: &[ToolDescriptor] = &[
    ToolDescriptor {
        id: ToolId::CreateTask,
        name: "create_task",
        capability: ToolCapability::TaskCreate,
        cli_noun: "task",
        cli_verb: "create",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::StartTask,
        name: "start_task",
        capability: ToolCapability::TaskStart,
        cli_noun: "task",
        cli_verb: "start",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::CompleteTask,
        name: "complete_task",
        capability: ToolCapability::TaskComplete,
        cli_noun: "task",
        cli_verb: "complete",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::MarkTaskGuardSatisfied,
        name: "mark_task_guard_satisfied",
        capability: ToolCapability::TaskComplete,
        cli_noun: "task",
        cli_verb: "guard",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::ReviseTask,
        name: "revise_task",
        capability: ToolCapability::TaskRevise,
        cli_noun: "task",
        cli_verb: "revise",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::AbandonTask,
        name: "abandon_task",
        capability: ToolCapability::TaskAbandon,
        cli_noun: "task",
        cli_verb: "abandon",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::ListTasks,
        name: "list_tasks",
        capability: ToolCapability::TaskRead,
        cli_noun: "task",
        cli_verb: "list",
        mutation: false,
    },
    ToolDescriptor {
        id: ToolId::AddFinding,
        name: "add_finding",
        capability: ToolCapability::FindingAdd,
        cli_noun: "finding",
        cli_verb: "add",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::RecordRubricScore,
        name: "record_rubric_score",
        capability: ToolCapability::RubricRecord,
        cli_noun: "rubric",
        cli_verb: "record",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::RecordNonNegotiableCompliance,
        name: "record_non_negotiable_compliance",
        capability: ToolCapability::ComplianceRecord,
        cli_noun: "compliance",
        cli_verb: "record",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::SetSpecTitle,
        name: "set_spec_title",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "set-title",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::SetSpecProblemStatement,
        name: "set_spec_problem_statement",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "set-problem-statement",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::SetSpecMotivations,
        name: "set_spec_motivations",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "set-motivations",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::SetSpecExpectations,
        name: "set_spec_expectations",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "set-expectations",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::SetSpecPlannedBehaviors,
        name: "set_spec_planned_behaviors",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "set-planned-behaviors",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::SetSpecImplementationPlan,
        name: "set_spec_implementation_plan",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "set-implementation-plan",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::SetSpecNonNegotiables,
        name: "set_spec_non_negotiables",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "set-non-negotiables",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::AddSpecAcceptanceCriterion,
        name: "add_spec_acceptance_criterion",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "add-acceptance-criterion",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::SetSpecDemoEnvironment,
        name: "set_spec_demo_environment",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "set-demo-environment",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::SetSpecDependencies,
        name: "set_spec_dependencies",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "set-dependencies",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::SetSpecBaseBranch,
        name: "set_spec_base_branch",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "set-base-branch",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::SetSpecRelevanceContext,
        name: "set_spec_relevance_context",
        capability: ToolCapability::SpecFrontmatter,
        cli_noun: "spec",
        cli_verb: "set-relevance-context",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::AddDemoStep,
        name: "add_demo_step",
        capability: ToolCapability::DemoFrontmatter,
        cli_noun: "demo",
        cli_verb: "add-step",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::MarkDemoStepSkip,
        name: "mark_demo_step_skip",
        capability: ToolCapability::DemoFrontmatter,
        cli_noun: "demo",
        cli_verb: "mark-step-skip",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::AppendDemoResult,
        name: "append_demo_result",
        capability: ToolCapability::DemoResults,
        cli_noun: "demo",
        cli_verb: "append-result",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::AddSignpost,
        name: "add_signpost",
        capability: ToolCapability::SignpostAdd,
        cli_noun: "signpost",
        cli_verb: "add",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::UpdateSignpostStatus,
        name: "update_signpost_status",
        capability: ToolCapability::SignpostUpdate,
        cli_noun: "signpost",
        cli_verb: "update-status",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::ReportPhaseOutcome,
        name: "report_phase_outcome",
        capability: ToolCapability::PhaseOutcome,
        cli_noun: "phase",
        cli_verb: "outcome",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::EscalateToBlocker,
        name: "escalate_to_blocker",
        capability: ToolCapability::PhaseEscalate,
        cli_noun: "phase",
        cli_verb: "escalate",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::PostReplyDirective,
        name: "post_reply_directive",
        capability: ToolCapability::FeedbackReply,
        cli_noun: "phase",
        cli_verb: "reply",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::CreateIssue,
        name: "create_issue",
        capability: ToolCapability::IssueCreate,
        cli_noun: "issue",
        cli_verb: "create",
        mutation: true,
    },
    ToolDescriptor {
        id: ToolId::ListRelevantStandards,
        name: "list_relevant_standards",
        capability: ToolCapability::StandardRead,
        cli_noun: "standard",
        cli_verb: "list",
        mutation: false,
    },
    ToolDescriptor {
        id: ToolId::RecordAdherenceFinding,
        name: "record_adherence_finding",
        capability: ToolCapability::AdherenceRecord,
        cli_noun: "adherence",
        cli_verb: "add-finding",
        mutation: true,
    },
];

/// Ordered descriptors for all methodology tools.
#[must_use]
pub const fn all_tool_descriptors() -> &'static [ToolDescriptor] {
    DESCRIPTORS
}

/// Descriptor by typed id.
#[must_use]
pub const fn descriptor(id: ToolId) -> &'static ToolDescriptor {
    match id {
        ToolId::CreateTask => &DESCRIPTORS[0],
        ToolId::StartTask => &DESCRIPTORS[1],
        ToolId::CompleteTask => &DESCRIPTORS[2],
        ToolId::MarkTaskGuardSatisfied => &DESCRIPTORS[3],
        ToolId::ReviseTask => &DESCRIPTORS[4],
        ToolId::AbandonTask => &DESCRIPTORS[5],
        ToolId::ListTasks => &DESCRIPTORS[6],
        ToolId::AddFinding => &DESCRIPTORS[7],
        ToolId::RecordRubricScore => &DESCRIPTORS[8],
        ToolId::RecordNonNegotiableCompliance => &DESCRIPTORS[9],
        ToolId::SetSpecTitle => &DESCRIPTORS[10],
        ToolId::SetSpecProblemStatement => &DESCRIPTORS[11],
        ToolId::SetSpecMotivations => &DESCRIPTORS[12],
        ToolId::SetSpecExpectations => &DESCRIPTORS[13],
        ToolId::SetSpecPlannedBehaviors => &DESCRIPTORS[14],
        ToolId::SetSpecImplementationPlan => &DESCRIPTORS[15],
        ToolId::SetSpecNonNegotiables => &DESCRIPTORS[16],
        ToolId::AddSpecAcceptanceCriterion => &DESCRIPTORS[17],
        ToolId::SetSpecDemoEnvironment => &DESCRIPTORS[18],
        ToolId::SetSpecDependencies => &DESCRIPTORS[19],
        ToolId::SetSpecBaseBranch => &DESCRIPTORS[20],
        ToolId::SetSpecRelevanceContext => &DESCRIPTORS[21],
        ToolId::AddDemoStep => &DESCRIPTORS[22],
        ToolId::MarkDemoStepSkip => &DESCRIPTORS[23],
        ToolId::AppendDemoResult => &DESCRIPTORS[24],
        ToolId::AddSignpost => &DESCRIPTORS[25],
        ToolId::UpdateSignpostStatus => &DESCRIPTORS[26],
        ToolId::ReportPhaseOutcome => &DESCRIPTORS[27],
        ToolId::EscalateToBlocker => &DESCRIPTORS[28],
        ToolId::PostReplyDirective => &DESCRIPTORS[29],
        ToolId::CreateIssue => &DESCRIPTORS[30],
        ToolId::ListRelevantStandards => &DESCRIPTORS[31],
        ToolId::RecordAdherenceFinding => &DESCRIPTORS[32],
    }
}

/// Resolve one descriptor by stable tool name.
#[must_use]
pub fn descriptor_by_name(name: &str) -> Option<&'static ToolDescriptor> {
    DESCRIPTORS.iter().find(|item| item.name == name)
}
