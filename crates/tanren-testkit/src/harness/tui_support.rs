//! PTY driver and screen-parsing helpers for the `@tui` wire harness.

use std::path::PathBuf;

use expectrl::Session;
use regex::Regex;
use tanren_contract::{
    AttentionSpecView, ProjectScopedViews, ProjectStateSummary, ProjectView, SwitchProjectResponse,
};
use tanren_identity_policy::{AccountId, ProjectId, SpecId};
use tanren_store::{ProjectStore, Store};

use super::{HarnessError, HarnessResult};

pub(super) struct TuiDriver {
    session: Session,
    output: Vec<u8>,
}

impl TuiDriver {
    pub(super) fn spawn(binary: &PathBuf, db_url: &str) -> HarnessResult<Self> {
        let mut cmd = std::process::Command::new(binary);
        cmd.env("DATABASE_URL", db_url);
        let session =
            Session::spawn(cmd).map_err(|e| HarnessError::Transport(format!("spawn tui: {e}")))?;
        Ok(Self {
            session,
            output: Vec::new(),
        })
    }

    pub(super) fn send(&mut self, data: &str) -> HarnessResult<()> {
        self.session
            .send(data)
            .map_err(|e| HarnessError::Transport(format!("send: {e}")))?;
        Ok(())
    }

    pub(super) fn wait_for(&mut self, pattern: &str) -> HarnessResult<()> {
        let found = self
            .session
            .expect(pattern)
            .map_err(|e| HarnessError::Transport(format!("expect '{pattern}': {e}")))?;
        self.output.extend_from_slice(found.before());
        self.output.extend_from_slice(pattern.as_bytes());
        Ok(())
    }

    pub(super) fn drain(&mut self) {
        std::thread::sleep(std::time::Duration::from_millis(400));
        if let Ok(found) = self.session.expect(expectrl::Regex("")) {
            self.output.extend_from_slice(found.before());
        }
    }

    pub(super) fn screen_text(&self) -> String {
        strip_ansi(&String::from_utf8_lossy(&self.output))
    }

    pub(super) fn navigate_to_sign_in(&mut self) -> HarnessResult<()> {
        self.send("\x1b[B")?;
        self.send("\r")?;
        self.wait_for("Sign in")?;
        Ok(())
    }

    pub(super) fn fill_sign_in(&mut self, email: &str, password: &str) -> HarnessResult<()> {
        self.send(email)?;
        self.send("\t")?;
        self.send(password)?;
        self.send("\r")?;
        Ok(())
    }

    pub(super) fn enter_project_list(&mut self) -> HarnessResult<()> {
        self.wait_for("account_id")?;
        self.send("\r")?;
        self.wait_for("Projects")?;
        self.drain();
        Ok(())
    }

    pub(super) fn enter_first_project(&mut self) -> HarnessResult<()> {
        self.send("\r")?;
        self.wait_for("State:")?;
        self.drain();
        Ok(())
    }

    pub(super) fn select_project_by_name(&mut self, name: &str) -> HarnessResult<()> {
        let plain = self.screen_text();
        let idx = parse_project_positions(&plain)
            .iter()
            .position(|n| n == name)
            .unwrap_or(0);
        for _ in 0..idx {
            self.send("\x1b[B")?;
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
        Ok(())
    }

    pub(super) fn navigate_to_spec(&mut self, spec_name: &str) -> HarnessResult<()> {
        let plain = self.screen_text();
        let idx = parse_spec_positions(&plain)
            .iter()
            .position(|n| n == spec_name)
            .unwrap_or(0);
        for _ in 0..idx {
            self.send("\x1b[B")?;
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
        Ok(())
    }
}

fn signin_and_enter_list(
    binary: &PathBuf,
    db_url: &str,
    email: &str,
    password: &str,
) -> HarnessResult<TuiDriver> {
    let mut drv = TuiDriver::spawn(binary, db_url)?;
    drv.wait_for("tanren-tui")?;
    drv.navigate_to_sign_in()?;
    drv.fill_sign_in(email, password)?;
    drv.enter_project_list()?;
    Ok(drv)
}

pub(super) fn drive_project_list(
    binary: &PathBuf,
    db_url: &str,
    email: &str,
    password: &str,
) -> HarnessResult<String> {
    let drv = signin_and_enter_list(binary, db_url, email, password)?;
    Ok(drv.screen_text())
}

pub(super) fn drive_switch_project(
    binary: &PathBuf,
    db_url: &str,
    email: &str,
    password: &str,
    project_name: &str,
) -> HarnessResult<String> {
    let mut drv = signin_and_enter_list(binary, db_url, email, password)?;
    drv.select_project_by_name(project_name)?;
    drv.send("\r")?;
    drv.wait_for("State:")?;
    drv.drain();
    Ok(drv.screen_text())
}

pub(super) fn drive_attention_spec(
    binary: &PathBuf,
    db_url: &str,
    email: &str,
    password: &str,
    spec_name: &str,
) -> HarnessResult<String> {
    let mut drv = signin_and_enter_list(binary, db_url, email, password)?;
    drv.enter_first_project()?;
    drv.navigate_to_spec(spec_name)?;
    drv.send("\r")?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    drv.drain();
    Ok(drv.screen_text())
}

pub(super) fn drive_scoped_views(
    binary: &PathBuf,
    db_url: &str,
    email: &str,
    password: &str,
) -> HarnessResult<String> {
    let mut drv = signin_and_enter_list(binary, db_url, email, password)?;
    drv.enter_first_project()?;
    Ok(drv.screen_text())
}

pub(super) fn parse_project_list(
    screen: &str,
    project_names: &std::collections::HashMap<ProjectId, String>,
    spec_data: &std::collections::HashMap<SpecId, (String, Option<String>)>,
) -> Vec<ProjectView> {
    let line_re =
        Regex::new(r"(?m)^\s*(>)?\s*(\S.*?)\s*\[(\w+)\](\s*\[!\])?").expect("project line regex");
    let mut projects = Vec::new();
    for cap in line_re.captures_iter(screen) {
        let name = cap.get(2).map_or("", |m| m.as_str()).trim().to_owned();
        let state_str = cap.get(3).map_or("active", |m| m.as_str());
        let has_attn = cap.get(4).is_some();
        let pid = project_names
            .iter()
            .find(|(_, n)| *n == &name)
            .map_or_else(ProjectId::fresh, |(&id, _)| id);
        let attn = if has_attn {
            collect_visible_specs(screen, spec_data)
        } else {
            vec![]
        };
        projects.push(ProjectView {
            id: pid,
            name,
            state: parse_state(state_str),
            needs_attention: has_attn,
            attention_specs: attn,
            created_at: chrono::Utc::now(),
        });
    }
    projects
}

pub(super) fn build_switch_response(
    screen: &str,
    target_pid: ProjectId,
    project_names: &std::collections::HashMap<ProjectId, String>,
    spec_data: &std::collections::HashMap<SpecId, (String, Option<String>)>,
    store: &Store,
) -> HarnessResult<SwitchProjectResponse> {
    let state_re = Regex::new(r"State:\s*(\w+)").expect("state regex");
    let state_str = state_re
        .captures(screen)
        .and_then(|c| c.get(1))
        .map_or("active", |m| m.as_str());
    let attn = collect_visible_specs(screen, spec_data);
    let name = project_names.get(&target_pid).cloned().unwrap_or_default();
    let project = ProjectView {
        id: target_pid,
        name,
        state: parse_state(state_str),
        needs_attention: !attn.is_empty(),
        attention_specs: attn,
        created_at: chrono::Utc::now(),
    };
    let scoped = tokio::runtime::Handle::current()
        .block_on(async { store.read_scoped_views(target_pid).await })
        .map_err(|e| HarnessError::Transport(format!("scoped: {e}")))?;
    Ok(SwitchProjectResponse {
        project,
        scoped: ProjectScopedViews {
            project_id: target_pid,
            specs: scoped.spec_ids,
            loops: scoped.loop_ids,
            milestones: scoped.milestone_ids,
        },
    })
}

pub(super) fn parse_attention_spec(
    screen: &str,
    sid: SpecId,
    spec_name: &str,
) -> AttentionSpecView {
    let reason_re = Regex::new(r"Reason:\s*(.+)").expect("reason regex");
    let reason = reason_re
        .captures(screen)
        .and_then(|c| c.get(1))
        .map_or(String::new(), |m| m.as_str().trim().to_owned());
    AttentionSpecView {
        id: sid,
        name: spec_name.to_owned(),
        reason,
    }
}

pub(super) fn read_active_project_scoped(
    store: &Store,
    tui_aid: AccountId,
) -> HarnessResult<ProjectScopedViews> {
    let active = tokio::runtime::Handle::current()
        .block_on(async { store.read_active_project(tui_aid).await })
        .map_err(|e| HarnessError::Transport(format!("active: {e}")))?
        .ok_or_else(|| HarnessError::Transport("no active project".to_owned()))?;
    let scoped = tokio::runtime::Handle::current()
        .block_on(async { store.read_scoped_views(active.project_id).await })
        .map_err(|e| HarnessError::Transport(format!("scoped: {e}")))?;
    Ok(ProjectScopedViews {
        project_id: active.project_id,
        specs: scoped.spec_ids,
        loops: scoped.loop_ids,
        milestones: scoped.milestone_ids,
    })
}

fn parse_project_positions(screen: &str) -> Vec<String> {
    let re = Regex::new(r"(?m)^\s*(>)?\s*(\S.*?)\s*\[\w+\]").expect("project pos regex");
    re.captures_iter(screen)
        .filter_map(|cap| cap.get(2).map(|m| m.as_str().trim().to_owned()))
        .collect()
}

fn parse_spec_positions(screen: &str) -> Vec<String> {
    let re = Regex::new(r"(?m)^\s*!\s+(\S+)").expect("spec pos regex");
    re.captures_iter(screen)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_owned()))
        .collect()
}

fn collect_visible_specs(
    screen: &str,
    spec_data: &std::collections::HashMap<SpecId, (String, Option<String>)>,
) -> Vec<AttentionSpecView> {
    let re = Regex::new(r"(?m)^\s+!\s+(\S+)").expect("visible spec regex");
    re.captures_iter(screen)
        .filter_map(|cap| {
            let name = cap.get(1)?.as_str().to_owned();
            let (sid, reason) = spec_data.iter().find(|(_, (n, _))| n == &name).map_or_else(
                || (SpecId::fresh(), String::new()),
                |(&id, (_, r))| (id, r.clone().unwrap_or_default()),
            );
            Some(AttentionSpecView {
                id: sid,
                name,
                reason,
            })
        })
        .collect()
}

fn strip_ansi(s: &str) -> String {
    let re =
        Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07]*\x07|\x1b[^\[\]]?").expect("ansi regex");
    re.replace_all(s, "").into_owned()
}

fn parse_state(s: &str) -> ProjectStateSummary {
    match s {
        "paused" => ProjectStateSummary::Paused,
        "done" | "completed" => ProjectStateSummary::Completed,
        "archived" => ProjectStateSummary::Archived,
        _ => ProjectStateSummary::Active,
    }
}
