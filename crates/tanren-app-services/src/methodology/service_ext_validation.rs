use tanren_contract::methodology::RecordRubricScoreParams;
use tanren_domain::TaskId;
use tanren_domain::methodology::finding::{Finding, FindingSeverity, FindingSource};
use tanren_domain::methodology::pillar::PillarScope;
use tanren_domain::methodology::rubric::RubricScore;

use super::errors::{MethodologyError, MethodologyResult};

pub(super) fn validate_rubric_scope(
    params: &RecordRubricScoreParams,
) -> MethodologyResult<Option<TaskId>> {
    match params.scope {
        PillarScope::Spec => {
            if params.scope_target_id.is_some() {
                return Err(MethodologyError::FieldValidation {
                    field_path: "/scope_target_id".into(),
                    expected: "absent for spec-scoped rubric scores".into(),
                    actual: format!("{:?}", params.scope_target_id),
                    remediation: "omit scope_target_id when scope=spec".into(),
                });
            }
            Ok(None)
        }
        PillarScope::Task => {
            let Some(raw) = params.scope_target_id.as_deref() else {
                return Err(MethodologyError::FieldValidation {
                    field_path: "/scope_target_id".into(),
                    expected: "task id string when scope=task".into(),
                    actual: "null".into(),
                    remediation: "set scope_target_id to the task id this rubric score evaluates"
                        .into(),
                });
            };
            let uuid =
                uuid::Uuid::parse_str(raw).map_err(|e| MethodologyError::FieldValidation {
                    field_path: "/scope_target_id".into(),
                    expected: "UUID task id".into(),
                    actual: raw.to_owned(),
                    remediation: e.to_string(),
                })?;
            Ok(Some(TaskId::from_uuid(uuid)))
        }
    }
}

pub(super) fn validate_supporting_findings(
    params: &RecordRubricScoreParams,
    record: &RubricScore,
    findings: &[Finding],
    task_scope_target: Option<TaskId>,
) -> MethodologyResult<()> {
    for (idx, finding) in findings.iter().enumerate() {
        let source_pillar = match &finding.source {
            FindingSource::Audit {
                pillar: Some(pillar),
                ..
            } => pillar.as_str(),
            FindingSource::Audit { pillar: None, .. } => {
                return Err(MethodologyError::FieldValidation {
                    field_path: format!("/supporting_finding_ids/{idx}"),
                    expected: "audit finding with matching pillar".into(),
                    actual: format!("finding {} has no pillar", finding.id),
                    remediation:
                        "record the finding with source.audit.pillar matching the rubric pillar"
                            .into(),
                });
            }
            _ => {
                return Err(MethodologyError::FieldValidation {
                    field_path: format!("/supporting_finding_ids/{idx}"),
                    expected: "audit-sourced finding".into(),
                    actual: format!("finding {} source is not audit", finding.id),
                    remediation:
                        "support rubric scores with audit findings linked to the same pillar".into(),
                });
            }
        };
        if source_pillar != record.pillar.as_str() {
            return Err(MethodologyError::FieldValidation {
                field_path: format!("/supporting_finding_ids/{idx}"),
                expected: format!("pillar `{}`", record.pillar.as_str()),
                actual: source_pillar.to_owned(),
                remediation:
                    "link only findings whose source.audit.pillar matches the scored pillar".into(),
            });
        }
        if !matches!(
            finding.severity,
            FindingSeverity::FixNow | FindingSeverity::Defer
        ) {
            return Err(MethodologyError::FieldValidation {
                field_path: format!("/supporting_finding_ids/{idx}"),
                expected: "severity fix_now or defer".into(),
                actual: finding.severity.to_string(),
                remediation: "use supporting findings with actionable severities (fix_now/defer)"
                    .into(),
            });
        }
        if let Some(task_id) = task_scope_target
            && finding.attached_task != Some(task_id)
        {
            return Err(MethodologyError::FieldValidation {
                field_path: format!("/supporting_finding_ids/{idx}"),
                expected: format!("finding attached_task={task_id}"),
                actual: format!("{:?}", finding.attached_task),
                remediation:
                    "for task-scoped scores, support with findings attached to the same task".into(),
            });
        }
    }
    if params.scope == PillarScope::Task && record.score < record.target && findings.is_empty() {
        return Err(MethodologyError::RubricInvariantViolated {
            pillar: record.pillar.as_str().to_owned(),
            score: record.score.get(),
            reason: "task-scoped score below target requires supporting findings".into(),
        });
    }
    Ok(())
}
