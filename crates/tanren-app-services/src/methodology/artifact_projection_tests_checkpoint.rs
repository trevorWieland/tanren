use super::*;

use chrono::Duration;
use tanren_domain::methodology::events::{
    FindingAdded, MethodologyEvent, NonNegotiableComplianceRecorded, RubricScoreRecorded,
    SignpostAdded, SignpostStatusUpdated, TaskStarted,
};
use tanren_domain::methodology::evidence::{AuditFrontmatter, AuditStatus, SignpostsFrontmatter};
use tanren_domain::methodology::finding::{Finding, FindingSeverity, FindingSource};
use tanren_domain::methodology::pillar::{PillarId, PillarScope, PillarScore};
use tanren_domain::methodology::rubric::{ComplianceStatus, NonNegotiableCompliance, RubricScore};
use tanren_domain::methodology::signpost::{Signpost, SignpostStatus};
use tanren_domain::{FindingId, SignpostId};
use uuid::Uuid;

fn finding_id() -> FindingId {
    FindingId::from_uuid(Uuid::parse_str("019da764-e015-7af1-9385-8a7b98995ea1").expect("uuid"))
}

fn signpost_id() -> SignpostId {
    SignpostId::from_uuid(Uuid::parse_str("019da764-e015-7af1-9385-8a7b98995eb1").expect("uuid"))
}

fn mk_finding(spec_id: SpecId, created_at: DateTime<Utc>) -> Finding {
    Finding {
        id: finding_id(),
        spec_id,
        severity: FindingSeverity::FixNow,
        title: ne("Guard regression detected"),
        description: "A required guard was bypassed".to_owned(),
        affected_files: vec!["src/lib.rs".to_owned()],
        line_numbers: vec![42],
        source: FindingSource::Audit {
            phase: tanren_domain::methodology::phase_id::PhaseId::try_new("audit-spec")
                .expect("phase"),
            pillar: Some(ne("security")),
        },
        attached_task: Some(task_id()),
        created_at,
    }
}

fn mk_rubric_score() -> RubricScore {
    RubricScore::try_new(
        PillarId::try_new("security").expect("pillar"),
        PillarScore::try_new(6).expect("score"),
        PillarScore::try_new(10).expect("score"),
        PillarScore::try_new(7).expect("score"),
        ne("Missing envelope freshness evidence"),
        vec![finding_id()],
    )
    .expect("rubric score")
}

fn mk_non_negotiable() -> NonNegotiableCompliance {
    NonNegotiableCompliance {
        name: ne("fail-closed-mcp"),
        status: ComplianceStatus::Fail,
        rationale: ne("MCP startup lacked capability envelope"),
    }
}

fn mk_signpost(spec_id: SpecId, created_at: DateTime<Utc>) -> Signpost {
    Signpost {
        id: signpost_id(),
        spec_id,
        task_id: Some(task_id()),
        status: SignpostStatus::Unresolved,
        problem: ne("Projection replay cost grows with event history"),
        evidence: ne("Latency spike observed in mutation finalize"),
        tried: vec!["profiled fold loop".to_owned()],
        solution: Some("incremental checkpointing".to_owned()),
        resolution: None,
        files_affected: vec!["artifact_projection.rs".to_owned()],
        created_at,
        updated_at: created_at,
    }
}

fn raw_json_lines(lines: &[PhaseEventLine]) -> Vec<String> {
    lines
        .iter()
        .map(|line| serde_json::to_string(line).expect("line json"))
        .collect()
}

fn processed_bytes_for(lines: &[String], processed_lines: usize) -> u64 {
    lines
        .iter()
        .take(processed_lines)
        .map(|line| u64::try_from(line.len() + 1).expect("line bytes"))
        .sum()
}

#[test]
fn render_from_lines_projects_audit_and_signposts_markdown() {
    let sid = spec_id();
    let t0 = ts(2026, 4, 21, 13, 0, 0);
    let t1 = ts(2026, 4, 21, 13, 1, 0);
    let t2 = ts(2026, 4, 21, 13, 2, 0);
    let t3 = ts(2026, 4, 21, 13, 3, 0);
    let t4 = ts(2026, 4, 21, 13, 4, 0);
    let t5 = ts(2026, 4, 21, 13, 5, 0);
    let mut lines = sample_setup_lines(sid, t0, t1);
    lines.push(mk_line(
        event_id("019da764-e015-7af1-9385-8a7b98995ea2"),
        sid,
        t2,
        "audit-spec",
        "add_finding",
        MethodologyEvent::FindingAdded(FindingAdded {
            finding: Box::new(mk_finding(sid, t2)),
            idempotency_key: None,
        }),
    ));
    lines.push(mk_line(
        event_id("019da764-e015-7af1-9385-8a7b98995ea3"),
        sid,
        t3,
        "audit-spec",
        "record_rubric_score",
        MethodologyEvent::RubricScoreRecorded(RubricScoreRecorded {
            spec_id: sid,
            scope: PillarScope::Spec,
            scope_target_id: None,
            score: mk_rubric_score(),
        }),
    ));
    lines.push(mk_line(
        event_id("019da764-e015-7af1-9385-8a7b98995ea4"),
        sid,
        t4,
        "audit-spec",
        "record_non_negotiable_compliance",
        MethodologyEvent::NonNegotiableComplianceRecorded(NonNegotiableComplianceRecorded {
            spec_id: sid,
            scope: PillarScope::Spec,
            compliance: mk_non_negotiable(),
        }),
    ));
    lines.push(mk_line(
        event_id("019da764-e015-7af1-9385-8a7b98995eb2"),
        sid,
        t4,
        "do-task",
        "add_signpost",
        MethodologyEvent::SignpostAdded(SignpostAdded {
            signpost: Box::new(mk_signpost(sid, t4)),
        }),
    ));
    lines.push(mk_line(
        event_id("019da764-e015-7af1-9385-8a7b98995eb3"),
        sid,
        t5,
        "do-task",
        "update_signpost_status",
        MethodologyEvent::SignpostStatusUpdated(SignpostStatusUpdated {
            signpost_id: signpost_id(),
            spec_id: sid,
            status: SignpostStatus::Resolved,
            resolution: Some("Incremental checkpoint landed".to_owned()),
        }),
    ));

    let rendered = render_from_lines(spec_id(), &lines, &[]).expect("render");
    assert!(rendered.audit_md.contains("# Audit"));
    assert!(rendered.audit_md.contains("Guard regression detected"));
    assert!(rendered.audit_md.contains("security: 6/10"));
    assert!(rendered.audit_md.contains("fail-closed-mcp: fail"));
    assert!(rendered.signposts_md.contains("# Signposts"));
    assert!(
        rendered
            .signposts_md
            .contains("Projection replay cost grows with event history")
    );
    assert!(rendered.signposts_md.contains("resolved"));

    let (audit_frontmatter, _) =
        AuditFrontmatter::parse_from_markdown(&rendered.audit_md).expect("audit frontmatter");
    assert_eq!(audit_frontmatter.fix_now_count, 1);
    assert_eq!(audit_frontmatter.status, AuditStatus::Fail);

    let (signposts_frontmatter, _) =
        SignpostsFrontmatter::parse_from_markdown(&rendered.signposts_md)
            .expect("signposts frontmatter");
    assert_eq!(signposts_frontmatter.entries.len(), 1);
    assert_eq!(
        signposts_frontmatter.entries[0].status,
        SignpostStatus::Resolved
    );
}

#[test]
fn generated_manifest_includes_all_nine_projected_artifacts() {
    let manifest = generated_artifact_manifest();
    assert_eq!(manifest.generated_artifacts.len(), 9);
    assert!(
        manifest
            .generated_artifacts
            .contains(&"audit.md".to_owned())
    );
    assert!(
        manifest
            .generated_artifacts
            .contains(&"signposts.md".to_owned())
    );
}

#[test]
fn checkpoint_incremental_append_matches_full_replay() {
    let sid = spec_id();
    let lines = sample_lines();
    let raw_lines_owned = raw_json_lines(&lines);
    let raw_lines = raw_lines_owned
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();

    let checkpoint_state = fold_projection_lines(sid, &lines[..2], &[]);
    let checkpoint = ProjectionCheckpoint {
        schema_version: PROJECTION_CHECKPOINT_SCHEMA_VERSION.to_owned(),
        contract_version: ARTIFACT_CONTRACT_VERSION.to_owned(),
        spec_id: sid,
        processed_lines: 2,
        processed_bytes: processed_bytes_for(&raw_lines_owned, 2),
        last_event_id: Some(lines[1].event_id),
        compacted_at: lines[1].timestamp,
        compacted_line_count: 2,
        state: checkpoint_state,
    };

    let (incremental, compacted_at) =
        fold_with_optional_checkpoint(sid, &raw_lines, &[], Some(checkpoint))
            .expect("incremental fold");
    let full = fold_projection_lines(sid, &lines, &[]);

    assert_eq!(incremental.latest_event_id, full.latest_event_id);
    assert_eq!(incremental.generated_at, full.generated_at);
    assert_eq!(incremental.tasks.len(), full.tasks.len());
    assert_eq!(incremental.tasks[0].task.status, full.tasks[0].task.status);
    assert_eq!(compacted_at, lines[1].timestamp);
}

#[test]
fn checkpoint_anchor_mismatch_falls_back_to_full_replay() {
    let sid = spec_id();
    let lines = sample_lines();
    let raw_lines_owned = raw_json_lines(&lines);
    let raw_lines = raw_lines_owned
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let checkpoint_state = fold_projection_lines(sid, &lines[..2], &[]);

    let bad_checkpoint = ProjectionCheckpoint {
        schema_version: PROJECTION_CHECKPOINT_SCHEMA_VERSION.to_owned(),
        contract_version: ARTIFACT_CONTRACT_VERSION.to_owned(),
        spec_id: sid,
        processed_lines: 2,
        processed_bytes: processed_bytes_for(&raw_lines_owned, 2),
        last_event_id: Some(EventId::new()),
        compacted_at: lines[1].timestamp,
        compacted_line_count: 2,
        state: checkpoint_state,
    };

    let (folded, compacted_at) =
        fold_with_optional_checkpoint(sid, &raw_lines, &[], Some(bad_checkpoint))
            .expect("fallback fold");
    let full = fold_projection_lines(sid, &lines, &[]);

    assert_eq!(folded.latest_event_id, full.latest_event_id);
    assert_eq!(folded.generated_at, full.generated_at);
    assert_eq!(compacted_at, full.generated_at);
}

#[test]
fn corrupted_checkpoint_is_ignored_and_full_replay_runs() {
    let sid = spec_id();
    let lines = sample_lines();
    let raw_lines_owned = raw_json_lines(&lines);
    let raw_lines = raw_lines_owned
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();

    let root = tempfile::tempdir().expect("tempdir");
    let checkpoint_path = root.path().join(PROJECTION_CHECKPOINT_FILE);
    std::fs::write(&checkpoint_path, "{ this is not valid json").expect("write corrupted");
    let prior = load_projection_checkpoint(&checkpoint_path);
    assert!(prior.is_none(), "corrupted checkpoint must be ignored");

    let (folded, _) = fold_with_optional_checkpoint(sid, &raw_lines, &[], prior).expect("fold");
    let full = fold_projection_lines(sid, &lines, &[]);
    assert_eq!(folded.latest_event_id, full.latest_event_id);
    assert_eq!(folded.tasks.len(), full.tasks.len());
}

#[test]
fn checkpoint_compacts_after_large_append_window() {
    let sid = spec_id();
    let t0 = ts(2026, 4, 21, 14, 0, 0);
    let t1 = ts(2026, 4, 21, 14, 1, 0);
    let mut lines = sample_setup_lines(sid, t0, t1);

    for idx in 0..=DEFAULT_CHECKPOINT_COMPACTION_APPEND_THRESHOLD {
        let offset = i64::try_from(idx).expect("append index should fit i64");
        let event_ts = t1 + Duration::seconds(offset + 1);
        lines.push(mk_line(
            EventId::new(),
            sid,
            event_ts,
            "do-task",
            "start_task",
            MethodologyEvent::TaskStarted(TaskStarted {
                task_id: task_id(),
                spec_id: sid,
            }),
        ));
    }

    let raw_lines_owned = raw_json_lines(&lines);
    let raw_lines = raw_lines_owned
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let checkpoint_state = fold_projection_lines(sid, &lines[..2], &[]);
    let checkpoint = ProjectionCheckpoint {
        schema_version: PROJECTION_CHECKPOINT_SCHEMA_VERSION.to_owned(),
        contract_version: ARTIFACT_CONTRACT_VERSION.to_owned(),
        spec_id: sid,
        processed_lines: 2,
        processed_bytes: processed_bytes_for(&raw_lines_owned, 2),
        last_event_id: Some(lines[1].event_id),
        compacted_at: t1,
        compacted_line_count: 2,
        state: checkpoint_state,
    };

    let (folded, compacted_at) =
        fold_with_optional_checkpoint(sid, &raw_lines, &[], Some(checkpoint))
            .expect("incremental fold");
    assert_eq!(compacted_at, folded.generated_at);
}
