use std::process::Stdio;

use async_trait::async_trait;
use chrono::Utc;
use regex::Regex;
use tanren_contract::{
    ActiveProjectRequest, ActiveProjectView, ConnectProjectRepositoryRequest,
    ConnectProjectRepositoryResponse, CreateProjectRequest, CreateProjectResponse,
    ListVisibleProjectsRequest, ProjectCollectionView, ProjectCountsView, ProjectRepositoryView,
    ProjectSelectionView, ProjectView,
};
use tanren_identity_policy::{AccountId, ProjectId, ProviderFamily, RepositoryRef};
use tokio::process::Command;
use uuid::Uuid;

use super::super::{HarnessError, HarnessResult, ProjectHarness};
use super::{CliHarness, translate_cli_error};

#[async_trait]
impl ProjectHarness for CliHarness {
    async fn connect_project_repository(
        &mut self,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        let owning_account_id = req.owning_account_id;
        let output = Command::new(&self.binary)
            .env("TANREN_SOURCE_CONTROL_PROVIDER_FIXTURE", "allow_all")
            .args([
                "project",
                "connect-repository",
                "--database-url",
                &self.db_url,
                "--owning-account-id",
                &owning_account_id.to_string(),
                "--repository",
                req.repository.as_str(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let project = parse_project_line(&stdout, owning_account_id)?;
        Ok(ConnectProjectRepositoryResponse { project })
    }

    async fn list_visible_projects(
        &mut self,
        req: ListVisibleProjectsRequest,
    ) -> HarnessResult<ProjectCollectionView> {
        let owning_account_id = req.owning_account_id;
        let output = Command::new(&self.binary)
            .args([
                "project",
                "list",
                "--database-url",
                &self.db_url,
                "--owning-account-id",
                &owning_account_id.to_string(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_project_collection(&stdout, owning_account_id)
    }

    async fn create_project(
        &mut self,
        req: CreateProjectRequest,
    ) -> HarnessResult<CreateProjectResponse> {
        if !req.select_as_active {
            return Err(HarnessError::Transport(
                "cli harness currently expects select_as_active=true".to_owned(),
            ));
        }
        let owning_account_id = req.owning_account_id;
        let output = Command::new(&self.binary)
            .env("TANREN_SOURCE_CONTROL_PROVIDER_FIXTURE", "allow_all")
            .args([
                "project",
                "create",
                "--database-url",
                &self.db_url,
                "--owning-account-id",
                &owning_account_id.to_string(),
                "--repository",
                req.repository.as_str(),
                "--designated-host",
                req.designated_host.as_str(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let project = parse_project_line(&stdout, owning_account_id)?;
        Ok(CreateProjectResponse { project })
    }

    async fn active_project(
        &mut self,
        req: ActiveProjectRequest,
    ) -> HarnessResult<ActiveProjectView> {
        let owning_account_id = req.owning_account_id;
        let output = Command::new(&self.binary)
            .args([
                "project",
                "active",
                "--database-url",
                &self.db_url,
                "--owning-account-id",
                &owning_account_id.to_string(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_active_project(&stdout, owning_account_id)
    }
}

fn parse_project_collection(
    stdout: &str,
    owning_account_id: AccountId,
) -> HarnessResult<ProjectCollectionView> {
    let mut projects = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("project_id=") {
            projects.push(parse_project_line(trimmed, owning_account_id)?);
        }
    }
    Ok(ProjectCollectionView {
        owning_account_id,
        projects,
    })
}

fn parse_active_project(
    stdout: &str,
    owning_account_id: AccountId,
) -> HarnessResult<ActiveProjectView> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("project_id=") {
            let project = parse_project_line(trimmed, owning_account_id)?;
            return Ok(ActiveProjectView {
                owning_account_id,
                active_project: Some(project),
            });
        }
        if trimmed.contains("active_project=none") {
            return Ok(ActiveProjectView {
                owning_account_id,
                active_project: None,
            });
        }
    }
    Err(HarnessError::Transport(format!(
        "could not parse active project from cli stdout: {stdout}"
    )))
}

fn parse_project_line(stdout: &str, owning_account_id: AccountId) -> HarnessResult<ProjectView> {
    let re = Regex::new(
        r"project_id=([0-9a-fA-F-]+)\s+repository=([a-zA-Z0-9._/-]+)\s+active=(true|false)\s+specs=(\d+)\s+milestones=(\d+)\s+initiatives=(\d+)",
    )
    .expect("constant regex");
    let captures = re
        .captures(stdout)
        .ok_or_else(|| HarnessError::Transport(format!("could not parse cli stdout: {stdout}")))?;
    let project_id_raw = captures.get(1).map_or("", |m| m.as_str());
    let repository_raw = captures.get(2).map_or("", |m| m.as_str());
    let active_raw = captures.get(3).map_or("", |m| m.as_str());
    let specs_raw = captures.get(4).map_or("", |m| m.as_str());
    let milestones_raw = captures.get(5).map_or("", |m| m.as_str());
    let initiatives_raw = captures.get(6).map_or("", |m| m.as_str());

    let id = ProjectId::try_from_uuid(
        Uuid::parse_str(project_id_raw)
            .map_err(|e| HarnessError::Transport(format!("parse project id: {e}")))?,
    )
    .map_err(|e| HarnessError::Transport(format!("parse project id: {e}")))?;
    let repository = RepositoryRef::parse(repository_raw)
        .map_err(|e| HarnessError::Transport(format!("parse repository ref: {e}")))?;
    let is_active = parse_bool(active_raw)?;
    let specs = specs_raw
        .parse::<u64>()
        .map_err(|e| HarnessError::Transport(format!("parse specs count: {e}")))?;
    let milestones = milestones_raw
        .parse::<u64>()
        .map_err(|e| HarnessError::Transport(format!("parse milestones count: {e}")))?;
    let initiatives = initiatives_raw
        .parse::<u64>()
        .map_err(|e| HarnessError::Transport(format!("parse initiatives count: {e}")))?;
    let now = Utc::now();
    Ok(ProjectView {
        id,
        owning_account_id,
        repository: ProjectRepositoryView {
            provider_family: ProviderFamily::source_control(),
            repository,
        },
        selection: ProjectSelectionView {
            is_active,
            selected_at: if is_active { Some(now) } else { None },
        },
        counts: ProjectCountsView {
            specs,
            milestones,
            initiatives,
        },
        created_at: now,
    })
}

fn parse_bool(value: &str) -> HarnessResult<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(HarnessError::Transport(format!(
            "parse boolean value from cli output: {other}"
        ))),
    }
}
