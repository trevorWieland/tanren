use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use tanren_app_services::project::{ProjectDependencyView, ProjectSpecView};
use tanren_contract::{
    ConnectProjectRequest, ConnectProjectResponse, DisconnectProjectRequest,
    DisconnectProjectResponse, ListProjectsResponse, ProjectDependencyResponse, ProjectView,
    ReconnectProjectResponse,
};
use tanren_identity_policy::{AccountId, OrgId, ProjectId, SpecId};
use tanren_store::{EventEnvelope, ProjectStore as _};
use tokio::process::Command;

use super::cli::CliHarness;
use super::project::{
    ProjectHarness, record_to_view, seed_account_via_store, seed_active_loop_via_store,
    seed_dependency_via_store, seed_spec_via_store,
};
use super::{HarnessError, HarnessKind, HarnessResult};

pub struct ProjectCliHarness {
    inner: CliHarness,
}

impl std::fmt::Debug for ProjectCliHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectCliHarness").finish_non_exhaustive()
    }
}

impl ProjectCliHarness {
    pub async fn spawn() -> HarnessResult<Self> {
        Ok(Self {
            inner: CliHarness::spawn().await?,
        })
    }

    async fn exec_cli(&self, args: &[&str]) -> HarnessResult<String> {
        let db_url = self.inner.db_url();
        let mut all_args: Vec<String> = vec!["project".to_owned()];
        all_args.extend(args.iter().map(ToString::to_string));
        all_args.push("--database-url".to_owned());
        all_args.push(db_url.to_owned());

        let output = Command::new(self.inner.binary_path())
            .args(&all_args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn cli: {e}")))?;
        if !output.status.success() {
            return Err(super::cli::translate_cli_error(&output.stderr));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[async_trait]
impl ProjectHarness for ProjectCliHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Cli
    }

    async fn connect_project(
        &mut self,
        req: ConnectProjectRequest,
    ) -> HarnessResult<ConnectProjectResponse> {
        let stdout = self
            .exec_cli(&[
                "connect",
                "--account-id",
                &req.account_id.to_string(),
                "--org-id",
                &req.org_id.to_string(),
                "--name",
                &req.name,
                "--repository-url",
                &req.repository_url,
            ])
            .await?;
        let project = parse_cli_project(&stdout)?;
        Ok(ConnectProjectResponse { project })
    }

    async fn disconnect_project(
        &mut self,
        req: DisconnectProjectRequest,
    ) -> HarnessResult<DisconnectProjectResponse> {
        let stdout = self
            .exec_cli(&[
                "disconnect",
                "--project-id",
                &req.project_id.to_string(),
                "--account-id",
                &req.account_id.to_string(),
            ])
            .await?;
        let re = regex::Regex::new(r"disconnected project_id=([0-9a-f-]+)").expect("regex");
        let caps = re
            .captures(&stdout)
            .ok_or_else(|| HarnessError::Transport(format!("parse disconnect: {stdout}")))?;
        let id_raw = caps.get(1).map_or("", |m| m.as_str());
        let project_id = id_raw
            .parse()
            .map(ProjectId::new)
            .map_err(|e| HarnessError::Transport(format!("parse id: {e}")))?;
        let remaining = self.list_projects(req.account_id).await?;
        let unresolved = parse_cli_unresolved(&stdout);
        Ok(DisconnectProjectResponse {
            project_id,
            account_projects: remaining.projects,
            unresolved_inbound_dependencies: unresolved,
        })
    }

    async fn list_projects(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ListProjectsResponse> {
        let stdout = self
            .exec_cli(&["list", "--account-id", &account_id.to_string()])
            .await?;
        let projects = parse_cli_project_list(&stdout);
        Ok(ListProjectsResponse { projects })
    }

    async fn reconnect_project(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<ReconnectProjectResponse> {
        let reconnected = Arc::clone(self.inner.store_handle())
            .reconnect_project(project_id)
            .await
            .map_err(|e| HarnessError::Transport(format!("reconnect: {e}")))?;
        Ok(ReconnectProjectResponse {
            project: record_to_view(&reconnected.project),
        })
    }

    async fn project_specs(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectSpecView>> {
        let stdout = self
            .exec_cli(&["specs", "--project-id", &project_id.to_string()])
            .await?;
        Ok(parse_cli_specs(&stdout))
    }

    async fn project_dependencies(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectDependencyView>> {
        let stdout = self
            .exec_cli(&["dependencies", "--project-id", &project_id.to_string()])
            .await?;
        Ok(parse_cli_deps(&stdout))
    }

    async fn seed_account(&mut self) -> HarnessResult<(AccountId, OrgId)> {
        seed_account_via_store(self.inner.store_handle()).await
    }

    async fn seed_spec(&mut self, project_id: ProjectId, title: String) -> HarnessResult<SpecId> {
        seed_spec_via_store(self.inner.store_handle(), project_id, title).await
    }

    async fn seed_dependency(
        &mut self,
        source: ProjectId,
        source_spec: SpecId,
        target: ProjectId,
    ) -> HarnessResult<()> {
        seed_dependency_via_store(self.inner.store_handle(), source, source_spec, target).await
    }

    async fn seed_active_loop(&mut self, project_id: ProjectId) -> HarnessResult<()> {
        seed_active_loop_via_store(self.inner.store_handle(), project_id).await
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        use tanren_store::AccountStore as _;
        self.inner
            .store_handle()
            .recent_events(limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }
}

fn parse_cli_project(stdout: &str) -> HarnessResult<ProjectView> {
    let re = regex::Regex::new(
        r"project_id=([0-9a-f-]+)\s+name=([^\s]+)\s+org_id=([0-9a-f-]+)\s+repository_url=([^\s]+)\s+connected_at=([^\s]+)"
    ).expect("regex");
    let caps = re
        .captures(stdout)
        .ok_or_else(|| HarnessError::Transport(format!("parse project: {stdout}")))?;
    let id: uuid::Uuid = caps
        .get(1)
        .expect("regex group 1")
        .as_str()
        .parse()
        .map_err(|e| HarnessError::Transport(format!("parse id: {e}")))?;
    let name = caps.get(2).expect("regex group 2").as_str().to_owned();
    let org_id: uuid::Uuid = caps
        .get(3)
        .expect("regex group 3")
        .as_str()
        .parse()
        .map_err(|e| HarnessError::Transport(format!("parse org: {e}")))?;
    let repo = caps.get(4).expect("regex group 4").as_str().to_owned();
    let at_str = caps.get(5).expect("regex group 5").as_str();
    let at = chrono::DateTime::parse_from_rfc3339(at_str)
        .map_err(|e| HarnessError::Transport(format!("parse date: {e}")))?;
    Ok(ProjectView {
        id: ProjectId::new(id),
        name,
        org_id: OrgId::new(org_id),
        repository_url: repo,
        connected_at: at.with_timezone(&Utc),
        disconnected_at: None,
    })
}

fn parse_cli_project_list(stdout: &str) -> Vec<ProjectView> {
    let re = regex::Regex::new(
        r"project_id=([0-9a-f-]+)\s+name=([^\s]+)\s+org_id=([0-9a-f-]+)\s+repository_url=([^\s]+)\s+connected_at=([^\s]+)"
    ).expect("regex");
    re.captures_iter(stdout)
        .filter_map(|caps| {
            let id: uuid::Uuid = caps.get(1)?.as_str().parse().ok()?;
            let name = caps.get(2)?.as_str().to_owned();
            let org_id: uuid::Uuid = caps.get(3)?.as_str().parse().ok()?;
            let repo = caps.get(4)?.as_str().to_owned();
            let at_str = caps.get(5)?.as_str();
            let at = chrono::DateTime::parse_from_rfc3339(at_str).ok()?;
            Some(ProjectView {
                id: ProjectId::new(id),
                name,
                org_id: OrgId::new(org_id),
                repository_url: repo,
                connected_at: at.with_timezone(&Utc),
                disconnected_at: None,
            })
        })
        .collect()
}

fn parse_cli_unresolved(stdout: &str) -> Vec<ProjectDependencyResponse> {
    let re = regex::Regex::new(
        r"unresolved source_project_id=([0-9a-f-]+)\s+source_spec_id=([0-9a-f-]+)\s+target_project_id=([0-9a-f-]+)"
    ).expect("regex");
    re.captures_iter(stdout)
        .filter_map(|caps| {
            let src: uuid::Uuid = caps.get(1)?.as_str().parse().ok()?;
            let spec: uuid::Uuid = caps.get(2)?.as_str().parse().ok()?;
            let tgt: uuid::Uuid = caps.get(3)?.as_str().parse().ok()?;
            Some(ProjectDependencyResponse {
                source_project_id: ProjectId::new(src),
                source_spec_id: SpecId::new(spec),
                unresolved_target_project_id: ProjectId::new(tgt),
                detected_at: Utc::now(),
            })
        })
        .collect()
}

fn parse_cli_specs(stdout: &str) -> Vec<ProjectSpecView> {
    let re = regex::Regex::new(
        r"spec_id=([0-9a-f-]+)\s+project_id=([0-9a-f-]+)\s+title=([^\s]+)\s+created_at=([^\s]+)",
    )
    .expect("regex");
    re.captures_iter(stdout)
        .filter_map(|caps| {
            let id: uuid::Uuid = caps.get(1)?.as_str().parse().ok()?;
            let pid: uuid::Uuid = caps.get(2)?.as_str().parse().ok()?;
            let title = caps.get(3)?.as_str().to_owned();
            let at_str = caps.get(4)?.as_str();
            let at = chrono::DateTime::parse_from_rfc3339(at_str).ok()?;
            Some(ProjectSpecView {
                id: SpecId::new(id),
                project_id: ProjectId::new(pid),
                title,
                created_at: at.with_timezone(&Utc),
            })
        })
        .collect()
}

fn parse_cli_deps(stdout: &str) -> Vec<ProjectDependencyView> {
    let re = regex::Regex::new(
        r"source_project_id=([0-9a-f-]+)\s+source_spec_id=([0-9a-f-]+)\s+target_project_id=([0-9a-f-]+)\s+status=(resolved|unresolved)"
    ).expect("regex");
    re.captures_iter(stdout)
        .filter_map(|caps| {
            let src: uuid::Uuid = caps.get(1)?.as_str().parse().ok()?;
            let spec: uuid::Uuid = caps.get(2)?.as_str().parse().ok()?;
            let tgt: uuid::Uuid = caps.get(3)?.as_str().parse().ok()?;
            let status = caps.get(4)?.as_str();
            Some(ProjectDependencyView {
                source_project_id: ProjectId::new(src),
                source_spec_id: SpecId::new(spec),
                target_project_id: ProjectId::new(tgt),
                resolved: status == "resolved",
                detected_at: Utc::now(),
            })
        })
        .collect()
}
