use chrono::{DateTime, Utc};
use tanren_domain::methodology::evidence::audit::{AuditFrontmatter, AuditKind, AuditStatus};
use tanren_domain::methodology::evidence::demo::{DemoEnvironmentProbe, DemoFrontmatter, DemoKind};
use tanren_domain::methodology::evidence::frontmatter::EvidenceSchemaVersion;
use tanren_domain::methodology::evidence::signposts::{
    SignpostEntry, SignpostsFrontmatter, SignpostsKind,
};
use tanren_domain::methodology::evidence::spec::{SpecFrontmatter, SpecKind};
use tanren_domain::methodology::finding::FindingSeverity;
use tanren_domain::methodology::rubric::ComplianceStatus;
use tanren_domain::{NonEmptyString, SpecId};

use super::artifact_projection_fold::FoldedProjectionState;
use super::artifact_projection_helpers::{
    render_audit_body, render_demo_body, render_signposts_body,
};
use super::errors::{MethodologyError, MethodologyResult};

pub(super) fn build_spec_frontmatter(
    spec_id: SpecId,
    folded: &FoldedProjectionState,
) -> SpecFrontmatter {
    let title =
        folded.spec_state.title.clone().unwrap_or_else(|| {
            NonEmptyString::try_new("Untitled spec").expect("non-empty literal")
        });
    let base_branch = folded
        .spec_state
        .base_branch
        .clone()
        .unwrap_or_else(|| NonEmptyString::try_new("main").expect("non-empty literal"));
    SpecFrontmatter {
        schema_version: EvidenceSchemaVersion::current(),
        kind: SpecKind::Spec,
        spec_id,
        title,
        problem_statement: folded.spec_state.problem_statement.clone(),
        motivations: folded.spec_state.motivations.clone(),
        expectations: folded.spec_state.expectations.clone(),
        planned_behaviors: folded.spec_state.planned_behaviors.clone(),
        implementation_plan: folded.spec_state.implementation_plan.clone(),
        non_negotiables: folded.spec_state.non_negotiables.clone(),
        acceptance_criteria: folded.spec_state.acceptance_criteria.clone(),
        demo_environment: folded.spec_state.demo_environment.clone(),
        dependencies: folded.spec_state.dependencies.clone(),
        base_branch,
        touched_symbols: vec![],
        relevance_context: folded.spec_state.relevance_context.clone(),
        created_at: folded.spec_state.created_at.unwrap_or(folded.generated_at),
    }
}

pub(super) fn render_demo_markdown(
    spec_id: SpecId,
    generated_at: DateTime<Utc>,
    folded: &FoldedProjectionState,
) -> MethodologyResult<String> {
    let demo_frontmatter = DemoFrontmatter {
        schema_version: EvidenceSchemaVersion::current(),
        kind: DemoKind::Demo,
        spec_id,
        environment: DemoEnvironmentProbe {
            probed_at: folded.last_demo_mutation.unwrap_or(generated_at),
            connections_verified: !folded.demo_steps.is_empty() || !folded.demo_results.is_empty(),
        },
        steps: folded.demo_steps.clone(),
        results: folded.demo_results.clone(),
    };
    demo_frontmatter
        .render_to_markdown(&render_demo_body(&demo_frontmatter))
        .map_err(|err| MethodologyError::Validation(err.to_string()))
}

pub(super) fn render_audit_markdown(
    spec_id: SpecId,
    generated_at: DateTime<Utc>,
    folded: &FoldedProjectionState,
) -> MethodologyResult<String> {
    let fix_now_count = u32::try_from(
        folded
            .finding_views
            .iter()
            .filter(|view| {
                view.status.is_open() && matches!(view.finding.severity, FindingSeverity::FixNow)
            })
            .count(),
    )
    .map_err(|_| MethodologyError::Validation("fix_now findings exceed u32".to_owned()))?;
    let has_failing_compliance = folded
        .non_negotiables_compliance
        .iter()
        .any(|item| matches!(item.status, ComplianceStatus::Fail));
    let has_failing_rubric = folded.rubric_scores.iter().any(|score| !score.is_passing());
    let audit_frontmatter = AuditFrontmatter {
        kind: AuditKind::Audit,
        spec_id,
        scope: folded.audit_scope,
        scope_target_id: folded.audit_scope_target_id.clone(),
        status: if fix_now_count == 0 && !has_failing_compliance && !has_failing_rubric {
            AuditStatus::Pass
        } else {
            AuditStatus::Fail
        },
        fix_now_count,
        rubric: folded.rubric_scores.clone(),
        non_negotiables_compliance: folded.non_negotiables_compliance.clone(),
        findings: folded.findings.iter().map(|finding| finding.id).collect(),
        generated_at,
    };
    audit_frontmatter
        .render_to_markdown(&render_audit_body(
            &folded.finding_views,
            &folded.rubric_scores,
            &folded.non_negotiables_compliance,
        ))
        .map_err(|err| MethodologyError::Validation(err.to_string()))
}

pub(super) fn render_signposts_markdown(
    spec_id: SpecId,
    folded: &FoldedProjectionState,
) -> MethodologyResult<String> {
    let signposts_frontmatter = SignpostsFrontmatter {
        kind: SignpostsKind::Signposts,
        spec_id,
        entries: folded
            .signposts
            .iter()
            .map(|signpost| SignpostEntry {
                id: signpost.id,
                task_id: signpost.task_id,
                status: signpost.status,
                problem: signpost.problem.clone(),
                evidence: signpost.evidence.clone(),
                tried: signpost.tried.clone(),
                solution: signpost.solution.clone(),
                resolution: signpost.resolution.clone(),
                files_affected: signpost.files_affected.clone(),
                created_at: signpost.created_at,
                updated_at: signpost.updated_at,
            })
            .collect(),
    };
    signposts_frontmatter
        .render_to_markdown(&render_signposts_body(&folded.signposts))
        .map_err(|err| MethodologyError::Validation(err.to_string()))
}
