//! Exhaustive snapshot matrix for every `MethodologyEvent` variant.

use chrono::{TimeZone, Utc};
use tanren_domain::methodology::events::{
    AdherenceFindingAdded, DemoFrontmatterUpdated, EvidenceSchemaError, FindingAdded, IssueCreated,
    MethodologyEvent, NonNegotiableComplianceRecorded, PhaseOutcomeReported,
    ReplyDirectiveRecorded, RubricScoreRecorded, SignpostAdded, SignpostStatusUpdated, SpecDefined,
    SpecFrontmatterUpdated, TaskAbandoned, TaskAdherent, TaskAudited, TaskCompleted, TaskCreated,
    TaskGateChecked, TaskImplemented, TaskRevised, TaskStarted, TaskXChecked,
    UnauthorizedArtifactEdit,
};
use tanren_domain::methodology::finding::{Finding, FindingSeverity, FindingSource, StandardRef};
use tanren_domain::methodology::frontmatter_patch::{DemoFrontmatterPatch, SpecFrontmatterPatch};
use tanren_domain::methodology::issue::{Issue, IssuePriority, IssueProvider, IssueRef};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::phase_outcome::{PhaseOutcome, ReplyDisposition};
use tanren_domain::methodology::pillar::{PillarId, PillarScope, PillarScore};
use tanren_domain::methodology::rubric::{ComplianceStatus, NonNegotiableCompliance, RubricScore};
use tanren_domain::methodology::signpost::{Signpost, SignpostStatus};
use tanren_domain::methodology::spec::{
    DemoEnvironment, Spec, SpecDependencies, SpecRelevanceContext,
};
use tanren_domain::methodology::task::{
    AcceptanceCriterion, RequiredGuard, Task, TaskAbandonDisposition, TaskOrigin, TaskStatus,
};
use tanren_domain::{FindingId, IssueId, NonEmptyString, SignpostId, SpecId, TaskId};

fn ne(s: &str) -> NonEmptyString {
    NonEmptyString::try_new(s).expect("non-empty")
}
fn phase(s: &str) -> PhaseId {
    PhaseId::try_new(s).expect("phase")
}
fn ts() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 2, 3, 4, 5, 6)
        .single()
        .expect("timestamp")
}

#[derive(Clone)]
struct Fixtures {
    spec_id: SpecId,
    task_id: TaskId,
    finding_id: FindingId,
    signpost_id: SignpostId,
    task: Task,
    finding: Finding,
    signpost: Signpost,
    issue: Issue,
    score: RubricScore,
    spec: Spec,
}

struct FixtureIds {
    spec: SpecId,
    task: TaskId,
    finding: FindingId,
    signpost: SignpostId,
    issue: IssueId,
}

fn fixture_ids() -> FixtureIds {
    FixtureIds {
        spec: SpecId::from_uuid(
            uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000a1").expect("uuid"),
        ),
        task: TaskId::from_uuid(
            uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000b2").expect("uuid"),
        ),
        finding: FindingId::from_uuid(
            uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000c3").expect("uuid"),
        ),
        signpost: SignpostId::from_uuid(
            uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000d4").expect("uuid"),
        ),
        issue: IssueId::from_uuid(
            uuid::Uuid::parse_str("00000000-0000-0000-0000-0000000000e5").expect("uuid"),
        ),
    }
}

fn fixture_task(ids: &FixtureIds) -> Task {
    Task {
        id: ids.task,
        spec_id: ids.spec,
        title: ne("task"),
        description: "desc".into(),
        acceptance_criteria: vec![AcceptanceCriterion {
            id: ne("ac-1"),
            description: ne("criterion"),
            measurable: ne("measurable"),
        }],
        origin: TaskOrigin::User,
        status: TaskStatus::Pending,
        depends_on: vec![],
        parent_task_id: None,
        created_at: ts(),
        updated_at: ts(),
    }
}

fn fixture_finding(ids: &FixtureIds) -> Finding {
    Finding {
        id: ids.finding,
        spec_id: ids.spec,
        severity: FindingSeverity::FixNow,
        title: ne("finding"),
        description: "desc".into(),
        affected_files: vec!["src/lib.rs".into()],
        line_numbers: vec![12, 13],
        source: FindingSource::Audit {
            phase: phase("audit-task"),
            pillar: Some(ne("security")),
        },
        attached_task: Some(ids.task),
        created_at: ts(),
    }
}

fn fixture_signpost(ids: &FixtureIds) -> Signpost {
    Signpost {
        id: ids.signpost,
        spec_id: ids.spec,
        task_id: Some(ids.task),
        status: SignpostStatus::Unresolved,
        problem: ne("problem"),
        evidence: ne("evidence"),
        tried: vec!["attempt-1".into()],
        solution: None,
        resolution: None,
        files_affected: vec!["src/lib.rs".into()],
        created_at: ts(),
        updated_at: ts(),
    }
}

fn fixture_issue(ids: &FixtureIds) -> Issue {
    Issue {
        id: ids.issue,
        origin_spec_id: ids.spec,
        title: ne("issue"),
        description: "desc".into(),
        suggested_spec_scope: ne("scope"),
        priority: IssuePriority::High,
        reference: IssueRef {
            provider: IssueProvider::GitHub,
            number: 42,
            url: ne("urn:tanren:issue:test"),
        },
        created_at: ts(),
    }
}

fn fixture_score(ids: &FixtureIds) -> RubricScore {
    RubricScore::try_new(
        PillarId::try_new("security").expect("pillar"),
        PillarScore::try_new(6).expect("score"),
        PillarScore::try_new(10).expect("target"),
        PillarScore::try_new(7).expect("passing"),
        ne("rationale"),
        vec![ids.finding],
    )
    .expect("score")
}

fn fixture_spec(ids: &FixtureIds) -> Spec {
    Spec {
        id: ids.spec,
        title: ne("spec"),
        non_negotiables: vec![ne("non-negotiable")],
        acceptance_criteria: vec![AcceptanceCriterion {
            id: ne("spec-ac"),
            description: ne("desc"),
            measurable: ne("measurable"),
        }],
        demo_environment: DemoEnvironment::default(),
        dependencies: SpecDependencies::default(),
        base_branch: ne("main"),
        touched_symbols: vec![],
        relevance_context: SpecRelevanceContext::default(),
        created_at: ts(),
    }
}

fn fixtures() -> Fixtures {
    let ids = fixture_ids();
    Fixtures {
        spec_id: ids.spec,
        task_id: ids.task,
        finding_id: ids.finding,
        signpost_id: ids.signpost,
        task: fixture_task(&ids),
        finding: fixture_finding(&ids),
        signpost: fixture_signpost(&ids),
        issue: fixture_issue(&ids),
        score: fixture_score(&ids),
        spec: fixture_spec(&ids),
    }
}

fn lifecycle_events(f: &Fixtures) -> Vec<MethodologyEvent> {
    vec![
        MethodologyEvent::SpecDefined(SpecDefined {
            spec: Box::new(f.spec.clone()),
        }),
        MethodologyEvent::TaskCreated(TaskCreated {
            task: Box::new(f.task.clone()),
            origin: TaskOrigin::User,
            idempotency_key: Some("k-task-created".into()),
        }),
        MethodologyEvent::TaskStarted(TaskStarted {
            task_id: f.task_id,
            spec_id: f.spec_id,
        }),
        MethodologyEvent::TaskImplemented(TaskImplemented {
            task_id: f.task_id,
            spec_id: f.spec_id,
            evidence_refs: vec!["evidence://one".into()],
        }),
        MethodologyEvent::TaskGateChecked(TaskGateChecked {
            task_id: f.task_id,
            spec_id: f.spec_id,
            idempotency_key: Some("k-gate".into()),
        }),
        MethodologyEvent::TaskAudited(TaskAudited {
            task_id: f.task_id,
            spec_id: f.spec_id,
            idempotency_key: Some("k-audit".into()),
        }),
        MethodologyEvent::TaskAdherent(TaskAdherent {
            task_id: f.task_id,
            spec_id: f.spec_id,
            idempotency_key: Some("k-adhere".into()),
        }),
        MethodologyEvent::TaskXChecked(TaskXChecked {
            task_id: f.task_id,
            spec_id: f.spec_id,
            guard_name: ne("x_guard"),
            idempotency_key: Some("k-x".into()),
        }),
        MethodologyEvent::TaskCompleted(TaskCompleted {
            task_id: f.task_id,
            spec_id: f.spec_id,
        }),
        MethodologyEvent::TaskAbandoned(TaskAbandoned {
            task_id: f.task_id,
            spec_id: f.spec_id,
            reason: ne("abandoned"),
            disposition: TaskAbandonDisposition::Replacement,
            replacements: vec![],
            explicit_user_discard_provenance: None,
        }),
        MethodologyEvent::TaskRevised(TaskRevised {
            task_id: f.task_id,
            spec_id: f.spec_id,
            revised_description: "new-desc".into(),
            revised_acceptance: vec![],
            reason: ne("because"),
        }),
    ]
}

fn finding_and_rubric_events(f: &Fixtures) -> Vec<MethodologyEvent> {
    vec![
        MethodologyEvent::FindingAdded(FindingAdded {
            finding: Box::new(f.finding.clone()),
            idempotency_key: Some("k-finding".into()),
        }),
        MethodologyEvent::AdherenceFindingAdded(AdherenceFindingAdded {
            finding: Box::new(Finding {
                source: FindingSource::Adherence {
                    standard: StandardRef {
                        name: ne("std"),
                        category: ne("cat"),
                    },
                },
                ..f.finding.clone()
            }),
            standard: StandardRef {
                name: ne("std"),
                category: ne("cat"),
            },
            idempotency_key: Some("k-adhere-finding".into()),
        }),
        MethodologyEvent::RubricScoreRecorded(RubricScoreRecorded {
            spec_id: f.spec_id,
            scope: PillarScope::Task,
            scope_target_id: Some(f.task_id.to_string()),
            score: f.score.clone(),
        }),
        MethodologyEvent::NonNegotiableComplianceRecorded(NonNegotiableComplianceRecorded {
            spec_id: f.spec_id,
            scope: PillarScope::Spec,
            compliance: NonNegotiableCompliance {
                name: ne("must-pass"),
                status: ComplianceStatus::Pass,
                rationale: ne("ok"),
            },
        }),
    ]
}

fn artifact_events(f: &Fixtures) -> Vec<MethodologyEvent> {
    vec![
        MethodologyEvent::SignpostAdded(SignpostAdded {
            signpost: Box::new(f.signpost.clone()),
        }),
        MethodologyEvent::SignpostStatusUpdated(SignpostStatusUpdated {
            signpost_id: f.signpost_id,
            spec_id: f.spec_id,
            status: SignpostStatus::Resolved,
            resolution: Some("resolved".into()),
        }),
        MethodologyEvent::IssueCreated(IssueCreated {
            issue: Box::new(f.issue.clone()),
            idempotency_key: Some("k-issue".into()),
        }),
        MethodologyEvent::PhaseOutcomeReported(PhaseOutcomeReported {
            spec_id: f.spec_id,
            phase: phase("audit-task"),
            agent_session_id: ne("session-1"),
            outcome: PhaseOutcome::Complete {
                summary: ne("done"),
                next_action_hint: Some(ne("next")),
            },
        }),
        MethodologyEvent::ReplyDirectiveRecorded(ReplyDirectiveRecorded {
            spec_id: f.spec_id,
            phase: phase("handle-feedback"),
            thread_ref: ne("thread-1"),
            disposition: ReplyDisposition::Ack,
            body: "thanks".into(),
        }),
        MethodologyEvent::SpecFrontmatterUpdated(SpecFrontmatterUpdated {
            spec_id: f.spec_id,
            patch: SpecFrontmatterPatch::SetTitle {
                title: ne("new title"),
            },
        }),
        MethodologyEvent::DemoFrontmatterUpdated(DemoFrontmatterUpdated {
            spec_id: f.spec_id,
            patch: DemoFrontmatterPatch::AppendResult {
                step_id: ne("step-1"),
                status: tanren_domain::methodology::evidence::DemoStatus::Pass,
                observed: ne("observed"),
            },
        }),
        MethodologyEvent::UnauthorizedArtifactEdit(UnauthorizedArtifactEdit {
            spec_id: f.spec_id,
            phase: phase("do-task"),
            file: "plan.md".into(),
            diff_preview: "line 1".into(),
            agent_session_id: ne("session-1"),
        }),
        MethodologyEvent::EvidenceSchemaError(EvidenceSchemaError {
            spec_id: f.spec_id,
            phase: phase("audit-task"),
            file: "audit.md".into(),
            error: ne("schema mismatch"),
        }),
    ]
}

#[test]
fn snapshot_all_methodology_event_variants() {
    let f = fixtures();
    let mut events = lifecycle_events(&f);
    events.extend(finding_and_rubric_events(&f));
    events.extend(artifact_events(&f));

    let json = serde_json::to_string_pretty(&events).expect("serialize");
    insta::assert_snapshot!(json);

    let required = [
        RequiredGuard::GateChecked,
        RequiredGuard::Audited,
        RequiredGuard::Adherent,
    ];
    let folded =
        tanren_domain::methodology::events::fold_task_status(f.task_id, &required, events.iter());
    assert!(matches!(
        folded,
        Some(TaskStatus::Complete | TaskStatus::Abandoned { .. })
    ));
    assert_eq!(f.finding.id, f.finding_id);
}
