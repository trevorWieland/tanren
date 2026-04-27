//! Typed methodology tool catalog.
//!
//! Single source for tool identity, capabilities, CLI routing, and mutation classification.

use super::capability::ToolCapability;

/// Stable identifier for one methodology tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolId {
    CreateTask,
    StartTask,
    CompleteTask,
    MarkTaskGuardSatisfied,
    ResetTaskGuards,
    ReviseTask,
    AbandonTask,
    ListTasks,
    AddFinding,
    ListFindings,
    ResolveFinding,
    ReopenFinding,
    DeferFinding,
    SupersedeFinding,
    RecordFindingStillOpen,
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
    StartCheckRun,
    RecordCheckResult,
    RecordCheckFailure,
    RecordInvestigationAttempt,
    ListInvestigationAttempts,
    LinkRootCauseToFinding,
    PostReplyDirective,
    CreateIssue,
    ListRelevantStandards,
    RecordAdherenceFinding,
}

impl ToolId {
    /// Ordered list of all methodology tools.
    #[must_use]
    #[rustfmt::skip]
    pub const fn all() -> &'static [Self] {
        &[
            Self::CreateTask, Self::StartTask, Self::CompleteTask, Self::MarkTaskGuardSatisfied,
            Self::ResetTaskGuards, Self::ReviseTask, Self::AbandonTask, Self::ListTasks,
            Self::AddFinding, Self::ListFindings, Self::ResolveFinding, Self::ReopenFinding,
            Self::DeferFinding, Self::SupersedeFinding, Self::RecordFindingStillOpen,
            Self::RecordRubricScore, Self::RecordNonNegotiableCompliance,
            Self::SetSpecTitle, Self::SetSpecProblemStatement, Self::SetSpecMotivations,
            Self::SetSpecExpectations, Self::SetSpecPlannedBehaviors, Self::SetSpecImplementationPlan,
            Self::SetSpecNonNegotiables, Self::AddSpecAcceptanceCriterion, Self::SetSpecDemoEnvironment,
            Self::SetSpecDependencies, Self::SetSpecBaseBranch, Self::SetSpecRelevanceContext,
            Self::AddDemoStep, Self::MarkDemoStepSkip, Self::AppendDemoResult, Self::AddSignpost,
            Self::UpdateSignpostStatus, Self::ReportPhaseOutcome, Self::EscalateToBlocker,
            Self::StartCheckRun, Self::RecordCheckResult, Self::RecordCheckFailure,
            Self::RecordInvestigationAttempt, Self::ListInvestigationAttempts, Self::LinkRootCauseToFinding,
            Self::PostReplyDirective, Self::CreateIssue,
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

const fn td(
    id: ToolId,
    name: &'static str,
    capability: ToolCapability,
    cli_noun: &'static str,
    cli_verb: &'static str,
    mutation: bool,
) -> ToolDescriptor {
    ToolDescriptor {
        id,
        name,
        capability,
        cli_noun,
        cli_verb,
        mutation,
    }
}

#[rustfmt::skip]
const DESCRIPTORS: &[ToolDescriptor] = &[
    td(ToolId::CreateTask, "create_task", ToolCapability::TaskCreate, "task", "create", true),
    td(ToolId::StartTask, "start_task", ToolCapability::TaskStart, "task", "start", true),
    td(ToolId::CompleteTask, "complete_task", ToolCapability::TaskComplete, "task", "complete", true),
    td(ToolId::MarkTaskGuardSatisfied, "mark_task_guard_satisfied", ToolCapability::TaskComplete, "task", "guard", true),
    td(ToolId::ResetTaskGuards, "reset_task_guards", ToolCapability::TaskComplete, "task", "reset-guards", true),
    td(ToolId::ReviseTask, "revise_task", ToolCapability::TaskRevise, "task", "revise", true),
    td(ToolId::AbandonTask, "abandon_task", ToolCapability::TaskAbandon, "task", "abandon", true),
    td(ToolId::ListTasks, "list_tasks", ToolCapability::TaskRead, "task", "list", false),
    td(ToolId::AddFinding, "add_finding", ToolCapability::FindingAdd, "finding", "add", true),
    td(ToolId::ListFindings, "list_findings", ToolCapability::FindingRead, "finding", "list", false),
    td(ToolId::ResolveFinding, "resolve_finding", ToolCapability::FindingLifecycle, "finding", "resolve", true),
    td(ToolId::ReopenFinding, "reopen_finding", ToolCapability::FindingLifecycle, "finding", "reopen", true),
    td(ToolId::DeferFinding, "defer_finding", ToolCapability::FindingLifecycle, "finding", "defer", true),
    td(ToolId::SupersedeFinding, "supersede_finding", ToolCapability::FindingLifecycle, "finding", "supersede", true),
    td(ToolId::RecordFindingStillOpen, "record_finding_still_open", ToolCapability::FindingLifecycle, "finding", "still-open", true),
    td(ToolId::RecordRubricScore, "record_rubric_score", ToolCapability::RubricRecord, "rubric", "record", true),
    td(ToolId::RecordNonNegotiableCompliance, "record_non_negotiable_compliance", ToolCapability::ComplianceRecord, "compliance", "record", true),
    td(ToolId::SetSpecTitle, "set_spec_title", ToolCapability::SpecFrontmatter, "spec", "set-title", true),
    td(ToolId::SetSpecProblemStatement, "set_spec_problem_statement", ToolCapability::SpecFrontmatter, "spec", "set-problem-statement", true),
    td(ToolId::SetSpecMotivations, "set_spec_motivations", ToolCapability::SpecFrontmatter, "spec", "set-motivations", true),
    td(ToolId::SetSpecExpectations, "set_spec_expectations", ToolCapability::SpecFrontmatter, "spec", "set-expectations", true),
    td(ToolId::SetSpecPlannedBehaviors, "set_spec_planned_behaviors", ToolCapability::SpecFrontmatter, "spec", "set-planned-behaviors", true),
    td(ToolId::SetSpecImplementationPlan, "set_spec_implementation_plan", ToolCapability::SpecFrontmatter, "spec", "set-implementation-plan", true),
    td(ToolId::SetSpecNonNegotiables, "set_spec_non_negotiables", ToolCapability::SpecFrontmatter, "spec", "set-non-negotiables", true),
    td(ToolId::AddSpecAcceptanceCriterion, "add_spec_acceptance_criterion", ToolCapability::SpecFrontmatter, "spec", "add-acceptance-criterion", true),
    td(ToolId::SetSpecDemoEnvironment, "set_spec_demo_environment", ToolCapability::SpecFrontmatter, "spec", "set-demo-environment", true),
    td(ToolId::SetSpecDependencies, "set_spec_dependencies", ToolCapability::SpecFrontmatter, "spec", "set-dependencies", true),
    td(ToolId::SetSpecBaseBranch, "set_spec_base_branch", ToolCapability::SpecFrontmatter, "spec", "set-base-branch", true),
    td(ToolId::SetSpecRelevanceContext, "set_spec_relevance_context", ToolCapability::SpecFrontmatter, "spec", "set-relevance-context", true),
    td(ToolId::AddDemoStep, "add_demo_step", ToolCapability::DemoFrontmatter, "demo", "add-step", true),
    td(ToolId::MarkDemoStepSkip, "mark_demo_step_skip", ToolCapability::DemoFrontmatter, "demo", "mark-step-skip", true),
    td(ToolId::AppendDemoResult, "append_demo_result", ToolCapability::DemoResults, "demo", "append-result", true),
    td(ToolId::AddSignpost, "add_signpost", ToolCapability::SignpostAdd, "signpost", "add", true),
    td(ToolId::UpdateSignpostStatus, "update_signpost_status", ToolCapability::SignpostUpdate, "signpost", "update-status", true),
    td(ToolId::ReportPhaseOutcome, "report_phase_outcome", ToolCapability::PhaseOutcome, "phase", "outcome", true),
    td(ToolId::EscalateToBlocker, "escalate_to_blocker", ToolCapability::PhaseEscalate, "phase", "escalate", true),
    td(ToolId::StartCheckRun, "start_check_run", ToolCapability::CheckRecord, "check", "start", true),
    td(ToolId::RecordCheckResult, "record_check_result", ToolCapability::CheckRecord, "check", "result", true),
    td(ToolId::RecordCheckFailure, "record_check_failure", ToolCapability::CheckRecord, "check", "failure", true),
    td(ToolId::RecordInvestigationAttempt, "record_investigation_attempt", ToolCapability::InvestigationRecord, "investigation", "record-attempt", true),
    td(ToolId::ListInvestigationAttempts, "list_investigation_attempts", ToolCapability::InvestigationRecord, "investigation", "list-attempts", false),
    td(ToolId::LinkRootCauseToFinding, "link_root_cause_to_finding", ToolCapability::InvestigationRecord, "investigation", "link-root-cause", true),
    td(ToolId::PostReplyDirective, "post_reply_directive", ToolCapability::FeedbackReply, "phase", "reply", true),
    td(ToolId::CreateIssue, "create_issue", ToolCapability::IssueCreate, "issue", "create", true),
    td(ToolId::ListRelevantStandards, "list_relevant_standards", ToolCapability::StandardRead, "standard", "list", false),
    td(ToolId::RecordAdherenceFinding, "record_adherence_finding", ToolCapability::AdherenceRecord, "adherence", "add-finding", true),
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
        ToolId::ResetTaskGuards => &DESCRIPTORS[4],
        ToolId::ReviseTask => &DESCRIPTORS[5],
        ToolId::AbandonTask => &DESCRIPTORS[6],
        ToolId::ListTasks => &DESCRIPTORS[7],
        ToolId::AddFinding => &DESCRIPTORS[8],
        ToolId::ListFindings => &DESCRIPTORS[9],
        ToolId::ResolveFinding => &DESCRIPTORS[10],
        ToolId::ReopenFinding => &DESCRIPTORS[11],
        ToolId::DeferFinding => &DESCRIPTORS[12],
        ToolId::SupersedeFinding => &DESCRIPTORS[13],
        ToolId::RecordFindingStillOpen => &DESCRIPTORS[14],
        ToolId::RecordRubricScore => &DESCRIPTORS[15],
        ToolId::RecordNonNegotiableCompliance => &DESCRIPTORS[16],
        ToolId::SetSpecTitle => &DESCRIPTORS[17],
        ToolId::SetSpecProblemStatement => &DESCRIPTORS[18],
        ToolId::SetSpecMotivations => &DESCRIPTORS[19],
        ToolId::SetSpecExpectations => &DESCRIPTORS[20],
        ToolId::SetSpecPlannedBehaviors => &DESCRIPTORS[21],
        ToolId::SetSpecImplementationPlan => &DESCRIPTORS[22],
        ToolId::SetSpecNonNegotiables => &DESCRIPTORS[23],
        ToolId::AddSpecAcceptanceCriterion => &DESCRIPTORS[24],
        ToolId::SetSpecDemoEnvironment => &DESCRIPTORS[25],
        ToolId::SetSpecDependencies => &DESCRIPTORS[26],
        ToolId::SetSpecBaseBranch => &DESCRIPTORS[27],
        ToolId::SetSpecRelevanceContext => &DESCRIPTORS[28],
        ToolId::AddDemoStep => &DESCRIPTORS[29],
        ToolId::MarkDemoStepSkip => &DESCRIPTORS[30],
        ToolId::AppendDemoResult => &DESCRIPTORS[31],
        ToolId::AddSignpost => &DESCRIPTORS[32],
        ToolId::UpdateSignpostStatus => &DESCRIPTORS[33],
        ToolId::ReportPhaseOutcome => &DESCRIPTORS[34],
        ToolId::EscalateToBlocker => &DESCRIPTORS[35],
        ToolId::StartCheckRun => &DESCRIPTORS[36],
        ToolId::RecordCheckResult => &DESCRIPTORS[37],
        ToolId::RecordCheckFailure => &DESCRIPTORS[38],
        ToolId::RecordInvestigationAttempt => &DESCRIPTORS[39],
        ToolId::ListInvestigationAttempts => &DESCRIPTORS[40],
        ToolId::LinkRootCauseToFinding => &DESCRIPTORS[41],
        ToolId::PostReplyDirective => &DESCRIPTORS[42],
        ToolId::CreateIssue => &DESCRIPTORS[43],
        ToolId::ListRelevantStandards => &DESCRIPTORS[44],
        ToolId::RecordAdherenceFinding => &DESCRIPTORS[45],
    }
}

/// Resolve one descriptor by stable tool name.
#[must_use]
pub fn descriptor_by_name(name: &str) -> Option<&'static ToolDescriptor> {
    DESCRIPTORS.iter().find(|item| item.name == name)
}
