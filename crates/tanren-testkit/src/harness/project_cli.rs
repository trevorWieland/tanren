use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use tanren_app_services::project::{ProjectDependencyView, ProjectSpecView};
use tanren_contract::{
    ConnectProjectResponse, DisconnectProjectRequest, DisconnectProjectResponse,
    ListProjectsResponse, ProjectDependenciesResponse, ProjectSpecsResponse,
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
        req: tanren_contract::ConnectProjectRequest,
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
        let response: ConnectProjectResponse = parse_json_line(&stdout)?;
        Ok(response)
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
        let response: DisconnectProjectResponse = parse_json_line(&stdout)?;
        Ok(response)
    }

    async fn list_projects(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ListProjectsResponse> {
        let stdout = self
            .exec_cli(&["list", "--account-id", &account_id.to_string()])
            .await?;
        let response: ListProjectsResponse = parse_json_line(&stdout)?;
        Ok(response)
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
        let response: ProjectSpecsResponse = parse_json_line(&stdout)?;
        Ok(response
            .specs
            .into_iter()
            .map(|s| ProjectSpecView {
                id: s.id,
                project_id: s.project_id,
                title: s.title,
                created_at: s.created_at,
            })
            .collect())
    }

    async fn project_dependencies(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectDependencyView>> {
        let stdout = self
            .exec_cli(&["dependencies", "--project-id", &project_id.to_string()])
            .await?;
        let response: ProjectDependenciesResponse = parse_json_line(&stdout)?;
        Ok(response
            .dependencies
            .into_iter()
            .map(|d| ProjectDependencyView {
                source_project_id: d.source_project_id,
                source_spec_id: d.source_spec_id,
                target_project_id: d.target_project_id,
                resolved: d.resolved,
                detected_at: d.detected_at,
            })
            .collect())
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

fn parse_json_line<T: serde::de::DeserializeOwned>(stdout: &str) -> HarnessResult<T> {
    let line = stdout
        .lines()
        .next()
        .ok_or_else(|| HarnessError::Transport("empty stdout".to_owned()))?;
    serde_json::from_str(line).map_err(|e| HarnessError::Transport(format!("parse JSON: {e}")))
}
