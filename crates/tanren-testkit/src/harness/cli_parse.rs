//! CLI stdout parsing helpers for the `@cli` wire harness.
//!
//! Extracted from `cli.rs` to keep that file under the line budget.

use regex::Regex;
use tanren_contract::{
    AttentionSpecView, ProjectScopedViews, ProjectStateSummary, ProjectView, SwitchProjectResponse,
};
use tanren_identity_policy::{LoopId, MilestoneId, ProjectId, SpecId};
use uuid::Uuid;

use super::{HarnessError, HarnessResult};

pub(super) fn parse_project_list(stdout: &str) -> HarnessResult<Vec<ProjectView>> {
    let re =
        Regex::new(r"id=([0-9a-fA-F-]+)\s+name=([^\s]+)\s+state=(\w+)\s+needs_attention=(\w+)")
            .expect("constant regex");
    let mut projects = Vec::new();
    for line in stdout.lines() {
        if let Some(caps) = re.captures(line) {
            let id = ProjectId::from(
                Uuid::parse_str(caps.get(1).map_or("", |m| m.as_str()))
                    .map_err(|e| HarnessError::Transport(format!("parse project id: {e}")))?,
            );
            let name = caps.get(2).map_or("", |m| m.as_str()).to_owned();
            let state_str = caps.get(3).map_or("active", |m| m.as_str());
            let needs_attention = caps.get(4).is_some_and(|m| m.as_str() == "true");
            projects.push(ProjectView {
                id,
                name,
                state: parse_project_state(state_str),
                needs_attention,
                attention_specs: Vec::new(),
                created_at: chrono::Utc::now(),
            });
        }
    }
    Ok(projects)
}

pub(super) fn parse_switch_response(
    stdout: &str,
    project_id: ProjectId,
) -> HarnessResult<SwitchProjectResponse> {
    let re = Regex::new(r"project_id=([0-9a-fA-F-]+)\s+name=([^\s]+)").expect("constant regex");
    let caps = re.captures(stdout).ok_or_else(|| {
        HarnessError::Transport(format!("could not parse switch response: {stdout}"))
    })?;
    let name = caps.get(2).map_or("", |m| m.as_str()).to_owned();
    Ok(SwitchProjectResponse {
        project: ProjectView {
            id: project_id,
            name,
            state: ProjectStateSummary::Active,
            needs_attention: false,
            attention_specs: Vec::new(),
            created_at: chrono::Utc::now(),
        },
        scoped: ProjectScopedViews {
            project_id,
            specs: Vec::new(),
            loops: Vec::new(),
            milestones: Vec::new(),
        },
    })
}

pub(super) fn parse_attention_spec(stdout: &str) -> HarnessResult<AttentionSpecView> {
    let re = Regex::new(r"spec_id=([0-9a-fA-F-]+)\s+name=([^\s]+)\s+reason=(.*)")
        .expect("constant regex");
    let caps = re.captures(stdout).ok_or_else(|| {
        HarnessError::Transport(format!("could not parse attention spec: {stdout}"))
    })?;
    Ok(AttentionSpecView {
        id: SpecId::from(
            Uuid::parse_str(caps.get(1).map_or("", |m| m.as_str()))
                .map_err(|e| HarnessError::Transport(format!("parse spec id: {e}")))?,
        ),
        name: caps.get(2).map_or("", |m| m.as_str()).to_owned(),
        reason: caps.get(3).map_or("", |m| m.as_str()).trim().to_owned(),
    })
}

pub(super) fn parse_scoped_views(stdout: &str) -> HarnessResult<ProjectScopedViews> {
    let project_re = Regex::new(r"project_id=([0-9a-fA-F-]+)").expect("constant regex");
    let caps = project_re.captures(stdout).ok_or_else(|| {
        HarnessError::Transport(format!("could not parse scoped views: {stdout}"))
    })?;
    let project_id = ProjectId::from(
        Uuid::parse_str(caps.get(1).map_or("", |m| m.as_str()))
            .map_err(|e| HarnessError::Transport(format!("parse project id: {e}")))?,
    );
    let spec_re = Regex::new(r"specs=\[([^\]]*)\]").expect("constant regex");
    let loop_re = Regex::new(r"loops=\[([^\]]*)\]").expect("constant regex");
    let milestone_re = Regex::new(r"milestones=\[([^\]]*)\]").expect("constant regex");
    let specs = parse_uuid_list::<SpecId>(&spec_re, stdout);
    let loops = parse_uuid_list::<LoopId>(&loop_re, stdout);
    let milestones = parse_uuid_list::<MilestoneId>(&milestone_re, stdout);
    Ok(ProjectScopedViews {
        project_id,
        specs,
        loops,
        milestones,
    })
}

fn parse_uuid_list<T>(re: &Regex, stdout: &str) -> Vec<T>
where
    T: From<Uuid>,
{
    let raw = re
        .captures(stdout)
        .and_then(|c| c.get(1))
        .map_or("", |m| m.as_str());
    if raw.trim().is_empty() {
        return Vec::new();
    }
    raw.split(',')
        .filter_map(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return None;
            }
            Uuid::parse_str(trimmed).ok().map(T::from)
        })
        .collect()
}

fn parse_project_state(raw: &str) -> ProjectStateSummary {
    match raw {
        "Paused" | "paused" => ProjectStateSummary::Paused,
        "Completed" | "completed" => ProjectStateSummary::Completed,
        "Archived" | "archived" => ProjectStateSummary::Archived,
        _ => ProjectStateSummary::Active,
    }
}
