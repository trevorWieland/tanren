use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use tanren_domain::NonEmptyString;
use tanren_domain::methodology::events::{DemoFrontmatterPatch, SpecFrontmatterPatch};
use tanren_domain::methodology::evidence::demo::{
    DemoFrontmatter, DemoResult, DemoStep, DemoStepMode,
};
use tanren_domain::methodology::evidence::spec::SpecFrontmatter;
use tanren_domain::methodology::task::{RequiredGuard, TaskOrigin, TaskStatus};

use super::artifact_projection::{
    GENERATED_ARTIFACT_MANIFEST_FILE, RenderedArtifacts, TaskCounts, TaskEvidence,
    TaskProjectionRow, TasksFrontmatter,
};
use super::errors::{MethodologyError, MethodologyResult};
use super::phase_events::PhaseEventLine;

pub(super) fn apply_spec_patch(
    state: &mut super::artifact_projection::SpecState,
    patch: &SpecFrontmatterPatch,
) {
    match patch {
        SpecFrontmatterPatch::SetTitle { title } => state.title = Some(title.clone()),
        SpecFrontmatterPatch::SetProblemStatement { problem_statement } => {
            state.problem_statement = Some(problem_statement.clone());
        }
        SpecFrontmatterPatch::SetMotivations { motivations } => {
            state.motivations.clone_from(motivations);
        }
        SpecFrontmatterPatch::SetExpectations { expectations } => {
            state.expectations.clone_from(expectations);
        }
        SpecFrontmatterPatch::SetPlannedBehaviors { planned_behaviors } => {
            state.planned_behaviors.clone_from(planned_behaviors);
        }
        SpecFrontmatterPatch::SetImplementationPlan {
            implementation_plan,
        } => {
            state.implementation_plan.clone_from(implementation_plan);
        }
        SpecFrontmatterPatch::SetNonNegotiables { items } => {
            state.non_negotiables.clone_from(items);
        }
        SpecFrontmatterPatch::AddAcceptanceCriterion { criterion } => {
            state.acceptance_criteria.push(criterion.clone());
        }
        SpecFrontmatterPatch::SetDemoEnvironment { demo_environment } => {
            state.demo_environment = demo_environment.clone();
        }
        SpecFrontmatterPatch::SetDependencies { dependencies } => {
            state.dependencies = dependencies.clone();
        }
        SpecFrontmatterPatch::SetBaseBranch { branch } => state.base_branch = Some(branch.clone()),
        SpecFrontmatterPatch::SetRelevanceContext { relevance_context } => {
            state.relevance_context = relevance_context.clone();
        }
    }
}

pub(super) fn apply_demo_patch(
    steps: &mut Vec<DemoStep>,
    results: &mut Vec<DemoResult>,
    line: &PhaseEventLine,
    patch: &DemoFrontmatterPatch,
) {
    match patch {
        DemoFrontmatterPatch::AddStep {
            id,
            mode,
            description,
            expected_observable,
        } => {
            if let Some(existing) = steps.iter_mut().find(|step| step.id == *id) {
                existing.mode = *mode;
                existing.description = description.clone();
                existing.expected_observable = expected_observable.clone();
                if !matches!(mode, DemoStepMode::Skip) {
                    existing.skip_reason = None;
                }
            } else {
                steps.push(DemoStep {
                    id: id.clone(),
                    mode: *mode,
                    description: description.clone(),
                    expected_observable: expected_observable.clone(),
                    skip_reason: None,
                });
            }
        }
        DemoFrontmatterPatch::MarkStepSkip { step_id, reason } => {
            if let Some(existing) = steps.iter_mut().find(|step| step.id == *step_id) {
                existing.mode = DemoStepMode::Skip;
                existing.skip_reason = Some(reason.clone());
            }
        }
        DemoFrontmatterPatch::AppendResult {
            step_id,
            status,
            observed,
        } => {
            results.push(DemoResult {
                run_id: line.event_id,
                ran_at: line.timestamp,
                step_id: step_id.clone(),
                status: *status,
                observed: observed.as_str().to_owned(),
            });
        }
    }
}

pub(super) fn update_guard(
    row: Option<&mut TaskProjectionRow>,
    required_guards: &[RequiredGuard],
    line: &PhaseEventLine,
    guard: &RequiredGuard,
) {
    let Some(row) = row else {
        return;
    };
    row.guards.set(guard, true);
    if let TaskStatus::Implemented { .. } = row.task.status {
        row.task.status = TaskStatus::Implemented {
            guards: row.guards.clone(),
        };
        row.task.updated_at = line.timestamp;
        if row.guards.satisfies(required_guards) {
            row.evidence = task_evidence(line, "required guards satisfied");
        } else {
            row.evidence = task_evidence(line, &format!("guard `{guard}` satisfied"));
        }
    }
}

pub(super) fn task_evidence(line: &PhaseEventLine, rationale: &str) -> TaskEvidence {
    TaskEvidence {
        event_id: line.event_id,
        timestamp: line.timestamp,
        phase: line.phase.clone(),
        tool: line.tool.clone(),
        rationale: rationale.to_owned(),
    }
}

pub(super) fn render_spec_body(frontmatter: &SpecFrontmatter) -> String {
    format!(
        "# Spec\n\n## Problem Statement\n{}\n\n## Motivations\n{}\n\n## Expectations\n{}\n\n## Planned Behaviors\n{}\n\n## Ordered Implementation Plan\n{}\n",
        frontmatter
            .problem_statement
            .as_ref()
            .map_or("_Not set._".into(), |v| v.as_str().to_owned()),
        render_markdown_list(&frontmatter.motivations),
        render_markdown_list(&frontmatter.expectations),
        render_markdown_list(&frontmatter.planned_behaviors),
        render_markdown_numbered_list(&frontmatter.implementation_plan),
    )
}

pub(super) fn render_demo_body(frontmatter: &DemoFrontmatter) -> String {
    let mut out = String::from("# Demo\n\n## Steps\n");
    if frontmatter.steps.is_empty() {
        out.push_str("_No demo steps declared._\n");
    } else {
        for step in &frontmatter.steps {
            let _ = writeln!(
                out,
                "- {} [{}] {}",
                step.id.as_str(),
                match step.mode {
                    DemoStepMode::Run => "RUN",
                    DemoStepMode::Skip => "SKIP",
                },
                step.description.as_str()
            );
        }
    }
    out
}

pub(super) fn render_plan_body(spec: &SpecFrontmatter, tasks: &[TaskProjectionRow]) -> String {
    let mut out = String::new();
    out.push_str("# Plan\n\n");
    out.push_str("## Problem Statement\n");
    out.push_str(
        &spec
            .problem_statement
            .as_ref()
            .map_or("_Not set._\n\n".into(), |v| format!("{}\n\n", v.as_str())),
    );
    out.push_str("## Motivations\n");
    let _ = write!(out, "{}\n\n", render_markdown_list(&spec.motivations));
    out.push_str("## Expectations\n");
    let _ = write!(out, "{}\n\n", render_markdown_list(&spec.expectations));
    out.push_str("## Ordered Implementation Plan\n");
    let _ = write!(
        out,
        "{}\n\n",
        render_markdown_numbered_list(&spec.implementation_plan)
    );
    out.push_str("## Tasks\n");
    out.push_str("| Task ID | Owner/Phase | Status | Status Rationale / Event |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for row in tasks {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} (`{}`) |",
            row.task.id,
            owner_phase_label(&row.task.origin),
            row.task.status.tag(),
            row.evidence.rationale,
            row.evidence.event_id
        );
    }
    out
}

pub(super) fn render_tasks_markdown(
    frontmatter: &TasksFrontmatter,
    tasks: &[TaskProjectionRow],
) -> Result<String, tanren_domain::methodology::evidence::FrontmatterError> {
    let mut body = String::from(
        "# Tasks\n\n| Task ID | Title | Owner/Phase | Status | Rationale / Event |\n| --- | --- | --- | --- | --- |\n",
    );
    for row in tasks {
        let _ = writeln!(
            body,
            "| {} | {} | {} | {} | {} (`{}`) |",
            row.task.id,
            row.task.title.as_str(),
            owner_phase_label(&row.task.origin),
            row.task.status.tag(),
            row.evidence.rationale,
            row.evidence.event_id
        );
    }
    tanren_domain::methodology::evidence::join(frontmatter, &body)
}

fn render_markdown_list(items: &[NonEmptyString]) -> String {
    if items.is_empty() {
        "_None declared._".into()
    } else {
        items
            .iter()
            .map(|item| format!("- {}", item.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn render_markdown_numbered_list(items: &[NonEmptyString]) -> String {
    if items.is_empty() {
        "_None declared._".into()
    } else {
        items
            .iter()
            .enumerate()
            .map(|(idx, item)| format!("{}. {}", idx + 1, item.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub(super) fn owner_phase_label(origin: &TaskOrigin) -> String {
    match origin {
        TaskOrigin::ShapeSpec => "shape-spec".into(),
        TaskOrigin::Investigation { source_phase, .. }
        | TaskOrigin::Audit { source_phase, .. }
        | TaskOrigin::SpecInvestigation { source_phase, .. } => source_phase.as_str().to_owned(),
        TaskOrigin::Adherence { .. } => "adhere-task".into(),
        TaskOrigin::Demo { .. } => "run-demo".into(),
        TaskOrigin::Feedback { .. } => "handle-feedback".into(),
        TaskOrigin::SpecAudit { .. } => "audit-spec".into(),
        TaskOrigin::CrossSpecIntent { .. } => "cross-spec-intent".into(),
        TaskOrigin::CrossSpecMerge { .. } => "cross-spec-merge".into(),
        TaskOrigin::User => "user".into(),
    }
}

pub(super) fn count_tasks(tasks: &[TaskProjectionRow]) -> TaskCounts {
    let mut counts = TaskCounts::default();
    for row in tasks {
        match row.task.status {
            TaskStatus::Pending => counts.pending += 1,
            TaskStatus::InProgress => counts.in_progress += 1,
            TaskStatus::Implemented { .. } => counts.implemented += 1,
            TaskStatus::Complete => counts.complete += 1,
            TaskStatus::Abandoned { .. } => counts.abandoned += 1,
        }
    }
    counts.total = tasks.len();
    counts
}

pub(super) fn write_artifacts(
    spec_folder: &Path,
    rendered: RenderedArtifacts,
) -> MethodologyResult<()> {
    let writes: [(&str, String); 7] = [
        ("spec.md", rendered.spec_md),
        ("plan.md", rendered.plan_md),
        ("tasks.md", rendered.tasks_md),
        ("tasks.json", rendered.tasks_json),
        ("demo.md", rendered.demo_md),
        ("progress.json", rendered.progress_json),
        (GENERATED_ARTIFACT_MANIFEST_FILE, rendered.manifest_json),
    ];
    for (name, body) in writes {
        write_atomic(&spec_folder.join(name), body.as_bytes())?;
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> MethodologyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| MethodologyError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut temp_path = PathBuf::from(path);
    let file_name = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("artifact");
    temp_path.set_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::now_v7()));
    let mut file = std::fs::File::create(&temp_path).map_err(|source| MethodologyError::Io {
        path: temp_path.clone(),
        source,
    })?;
    file.write_all(bytes)
        .map_err(|source| MethodologyError::Io {
            path: temp_path.clone(),
            source,
        })?;
    file.sync_all().map_err(|source| MethodologyError::Io {
        path: temp_path.clone(),
        source,
    })?;
    std::fs::rename(&temp_path, path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_phase_labels_shape_spec() {
        assert_eq!(owner_phase_label(&TaskOrigin::ShapeSpec), "shape-spec");
    }
}
