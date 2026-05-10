use std::process::Stdio;

use async_trait::async_trait;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tanren_contract::{
    ActiveProjectRequest, ActiveProjectView, ConnectProjectRepositoryRequest,
    ConnectProjectRepositoryResponse, CreateProjectRequest, CreateProjectResponse,
    ListVisibleProjectsRequest, ProjectCollectionView,
};
use tanren_identity_policy::{AccountId, DesignatedHost, RepositoryRef};
use tokio::process::Command;

use super::super::api::project_code_to_reason;
use super::super::{HarnessError, HarnessResult, ProjectHarness};
use super::{CliHarness, translate_cli_error};

#[derive(Debug, Deserialize)]
struct CliProjectFailure {
    code: String,
    summary: String,
}

#[async_trait]
impl ProjectHarness for CliHarness {
    async fn connect_project_repository(
        &mut self,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        self.connect_project_repository_as_actor(req.owning_account_id, req)
            .await
    }

    async fn connect_project_repository_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        let session_token = session_token_for_actor(self, actor_account_id)?;
        let owning_account_id = req.owning_account_id.to_string();
        let output = run_project_command(
            self,
            [
                "connect-repository",
                "--owning-account-id",
                owning_account_id.as_str(),
                "--session-token",
                session_token.as_str(),
                "--repository",
                req.repository.as_str(),
            ],
            true,
        )
        .await?;
        if !output.status.success() {
            return Err(project_failure_from_output(&output.stdout, &output.stderr));
        }
        decode_project_json(&output.stdout, "decode connect project response")
    }

    async fn list_visible_projects(
        &mut self,
        req: ListVisibleProjectsRequest,
    ) -> HarnessResult<ProjectCollectionView> {
        self.list_visible_projects_as_actor(req.owning_account_id, req)
            .await
    }

    async fn list_visible_projects_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: ListVisibleProjectsRequest,
    ) -> HarnessResult<ProjectCollectionView> {
        let session_token = session_token_for_actor(self, actor_account_id)?;
        let owning_account_id = req.owning_account_id.to_string();
        let output = run_project_command(
            self,
            [
                "list",
                "--owning-account-id",
                owning_account_id.as_str(),
                "--session-token",
                session_token.as_str(),
            ],
            false,
        )
        .await?;
        if !output.status.success() {
            return Err(project_failure_from_output(&output.stdout, &output.stderr));
        }
        decode_project_json(&output.stdout, "decode list projects response")
    }

    async fn create_project(
        &mut self,
        req: CreateProjectRequest,
    ) -> HarnessResult<CreateProjectResponse> {
        self.create_project_as_actor(req.owning_account_id, req)
            .await
    }

    async fn create_project_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: CreateProjectRequest,
    ) -> HarnessResult<CreateProjectResponse> {
        if !req.select_as_active {
            return Err(HarnessError::Transport(
                "cli harness currently expects select_as_active=true".to_owned(),
            ));
        }
        let session_token = session_token_for_actor(self, actor_account_id)?;
        let designated_host = req.designated_host.clone();
        let owning_account_id = req.owning_account_id.to_string();
        let output = run_project_command(
            self,
            [
                "create",
                "--owning-account-id",
                owning_account_id.as_str(),
                "--session-token",
                session_token.as_str(),
                "--repository",
                req.repository.as_str(),
                "--designated-host",
                req.designated_host.as_str(),
            ],
            true,
        )
        .await?;
        if !output.status.success() {
            return Err(project_failure_from_output(&output.stdout, &output.stderr));
        }
        let response: CreateProjectResponse =
            decode_project_json(&output.stdout, "decode create project response")?;
        self.record_created_repository(&designated_host, &response.project.repository.repository);
        Ok(response)
    }

    async fn active_project(
        &mut self,
        req: ActiveProjectRequest,
    ) -> HarnessResult<ActiveProjectView> {
        self.active_project_as_actor(req.owning_account_id, req)
            .await
    }

    async fn active_project_as_actor(
        &mut self,
        actor_account_id: AccountId,
        req: ActiveProjectRequest,
    ) -> HarnessResult<ActiveProjectView> {
        let session_token = session_token_for_actor(self, actor_account_id)?;
        let owning_account_id = req.owning_account_id.to_string();
        let output = run_project_command(
            self,
            [
                "active",
                "--owning-account-id",
                owning_account_id.as_str(),
                "--session-token",
                session_token.as_str(),
            ],
            false,
        )
        .await?;
        if !output.status.success() {
            return Err(project_failure_from_output(&output.stdout, &output.stderr));
        }
        decode_project_json(&output.stdout, "decode active project response")
    }

    async fn set_repository_access(
        &mut self,
        actor_account_id: AccountId,
        repository: RepositoryRef,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.configure_repository_access(actor_account_id, repository, allowed);
        Ok(())
    }

    async fn set_designated_host_create_access(
        &mut self,
        actor_account_id: AccountId,
        host: DesignatedHost,
        allowed: bool,
    ) -> HarnessResult<()> {
        self.configure_designated_host_create_access(actor_account_id, &host, allowed);
        Ok(())
    }

    async fn repository_created_at_host(
        &self,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> HarnessResult<bool> {
        Ok(self.observed_repository_created_at_host(host, repository))
    }
}

fn decode_project_json<T: DeserializeOwned>(stdout: &[u8], context: &str) -> HarnessResult<T> {
    let payload = extract_last_output_line(stdout);
    serde_json::from_str(&payload).map_err(|e| {
        HarnessError::Transport(format!(
            "{context}: {e}; payload={payload}; stdout={}",
            String::from_utf8_lossy(stdout),
        ))
    })
}

fn project_failure_from_output(stdout: &[u8], stderr: &[u8]) -> HarnessError {
    let payload = extract_last_output_line(stdout);
    if let Ok(failure) = serde_json::from_str::<CliProjectFailure>(&payload) {
        if let Some(reason) = project_code_to_reason(&failure.code) {
            return HarnessError::Project(reason, failure.summary);
        }
        return HarnessError::Transport(format!("{}: {}", failure.code, failure.summary));
    }
    translate_cli_error(stderr)
}

fn extract_last_output_line(stdout: &[u8]) -> String {
    let text = String::from_utf8_lossy(stdout);
    text.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
        .unwrap_or_default()
}

fn session_token_for_actor(
    harness: &CliHarness,
    actor_account_id: AccountId,
) -> HarnessResult<String> {
    harness
        .session_token_for(actor_account_id)
        .map(str::to_owned)
        .ok_or_else(|| {
            HarnessError::Transport(format!(
                "missing CLI session token for actor account {actor_account_id}"
            ))
        })
}

async fn run_project_command<const N: usize>(
    harness: &CliHarness,
    args: [&str; N],
    with_provider_fixture: bool,
) -> HarnessResult<std::process::Output> {
    let mut command = Command::new(&harness.binary);
    command
        .env("TANREN_SESSION_FILE", &harness.session_file)
        .args(["project"])
        .args(args)
        .arg("--database-url")
        .arg(&harness.db_url)
        .arg("--output")
        .arg("json")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if with_provider_fixture {
        command.env(
            "TANREN_SOURCE_CONTROL_PROVIDER_FIXTURE",
            harness.project_provider_fixture_env_value(),
        );
    }
    command
        .output()
        .await
        .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))
}
