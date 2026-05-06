//! `@cli` harness — shells out to the `tanren-cli` binary against a
//! per-scenario `SQLite` file.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use regex::Regex;
use secrecy::ExposeSecret;
use serde_json::Value;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, AttentionSpecView, ProjectScopedViews, ProjectView,
    SignInRequest, SignUpRequest, SwitchProjectResponse,
};
use tanren_identity_policy::{AccountId, Identifier, OrgId, ProjectId, SpecId};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation, NewProject, NewSpec, ProjectStore};
use tokio::process::Command;
use uuid::Uuid;

use super::api_support::{
    code_to_project_reason, code_to_reason, locate_workspace_binary, scenario_db_path, sqlite_url,
};
use super::cli_parse::{
    parse_attention_spec, parse_project_list, parse_scoped_views, parse_switch_response,
};
use super::project::{HarnessProjectFixture, HarnessSpecFixture, ProjectHarness};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

pub struct CliHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    db_url: String,
    binary: PathBuf,
}

impl std::fmt::Debug for CliHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CliHarness")
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl CliHarness {
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = scenario_db_path("cli");
        let db_url = sqlite_url(&db_path);
        let store = Store::connect(&db_url)
            .await
            .map_err(|e| HarnessError::Transport(format!("connect: {e}")))?;
        store
            .migrate()
            .await
            .map_err(|e| HarnessError::Transport(format!("migrate: {e}")))?;
        let binary = locate_workspace_binary("tanren-cli")?;
        Ok(Self {
            store: Arc::new(store),
            db_path,
            db_url,
            binary,
        })
    }
}

impl Drop for CliHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.db_path);
    }
}

#[async_trait]
impl AccountHarness for CliHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Cli
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let output = run_cli(
            &self.binary,
            &[
                "account",
                "create",
                "--database-url",
                &self.db_url,
                "--identifier",
                req.email.as_str(),
                "--password",
                req.password.expose_secret(),
                "--display-name",
                &req.display_name,
            ],
        )
        .await?;
        let (account, has_token) = parse_session(&output, req.email.as_str(), &req.display_name)?;
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at: Utc::now() + Duration::days(30),
            has_token,
        })
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let output = run_cli(
            &self.binary,
            &[
                "account",
                "sign-in",
                "--database-url",
                &self.db_url,
                "--identifier",
                req.email.as_str(),
                "--password",
                req.password.expose_secret(),
            ],
        )
        .await?;
        let (account, has_token) = parse_session(&output, req.email.as_str(), "")?;
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at: Utc::now() + Duration::days(30),
            has_token,
        })
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let output = run_cli(
            &self.binary,
            &[
                "account",
                "create",
                "--database-url",
                &self.db_url,
                "--identifier",
                req.email.as_str(),
                "--password",
                req.password.expose_secret(),
                "--display-name",
                &req.display_name,
                "--invitation",
                req.invitation_token.as_str(),
            ],
        )
        .await?;
        let (account, has_token) = parse_session(&output, req.email.as_str(), &req.display_name)?;
        let joined_org = parse_joined_org(&output)?;
        let account = AccountView {
            org: Some(joined_org),
            ..account
        };
        Ok(HarnessAcceptance {
            session: HarnessSession {
                account_id: account.id,
                account,
                expires_at: Utc::now() + Duration::days(30),
                has_token,
            },
            joined_org,
        })
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.store
            .seed_invitation(NewInvitation {
                token: fixture.token,
                inviting_org_id: fixture.inviting_org,
                expires_at: fixture.expires_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed: {e}")))?;
        Ok(())
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("events: {e}")))
    }
}

async fn run_cli(binary: &PathBuf, args: &[&str]) -> HarnessResult<String> {
    let output = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| HarnessError::Transport(format!("cli: {e}")))?;
    if !output.status.success() {
        return Err(translate_cli_error(&output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn translate_cli_error(stderr: &[u8]) -> HarnessError {
    let text = String::from_utf8_lossy(stderr);
    let re = Regex::new(r"error:\s*([a-z_]+)\s*[-—]\s*(.*)").expect("const regex");
    if let Some(caps) = re.captures(&text) {
        let code = caps.get(1).map_or("", |m| m.as_str());
        let summary = caps.get(2).map_or("", |m| m.as_str()).trim().to_owned();
        if let Some(r) = code_to_reason(code) {
            return HarnessError::Account(r, summary);
        }
        if let Some(r) = code_to_project_reason(code) {
            return HarnessError::Project(r, summary);
        }
    }
    HarnessError::Transport(text.into_owned())
}

fn parse_session(
    stdout: &str,
    email: &str,
    display_name: &str,
) -> HarnessResult<(AccountView, bool)> {
    let re = Regex::new(r"account_id=([0-9a-fA-F-]+)\s+session=([^\s]+)").expect("const regex");
    let caps = re
        .captures(stdout)
        .ok_or_else(|| HarnessError::Transport(format!("parse cli: {stdout}")))?;
    let id = AccountId::from(
        Uuid::parse_str(caps.get(1).map_or("", |m| m.as_str()))
            .map_err(|e| HarnessError::Transport(format!("id: {e}")))?,
    );
    let token = caps.get(2).map_or("", |m| m.as_str());
    let identifier = Identifier::from_email(
        &tanren_identity_policy::Email::parse(email)
            .map_err(|e| HarnessError::Transport(format!("email: {e}")))?,
    );
    Ok((
        AccountView {
            id,
            identifier,
            display_name: if display_name.is_empty() {
                String::new()
            } else {
                display_name.to_owned()
            },
            org: None,
        },
        !token.is_empty(),
    ))
}

fn parse_joined_org(stdout: &str) -> HarnessResult<OrgId> {
    let re = Regex::new(r"joined_org=([0-9a-fA-F-]+)").expect("const regex");
    let raw = re
        .captures(stdout)
        .and_then(|c| c.get(1).map(|m| m.as_str()))
        .ok_or_else(|| HarnessError::Transport(format!("joined_org: {stdout}")))?;
    Ok(OrgId::from(Uuid::parse_str(raw).map_err(|e| {
        HarnessError::Transport(format!("org: {e}"))
    })?))
}

#[async_trait]
impl ProjectHarness for CliHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Cli
    }

    async fn seed_project(&mut self, f: HarnessProjectFixture) -> HarnessResult<ProjectId> {
        let id = f.id;
        self.store
            .seed_project(NewProject {
                id,
                account_id: f.account_id,
                name: f.name,
                state: f.state,
                created_at: f.created_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_project: {e}")))?;
        Ok(id)
    }

    async fn seed_spec(&mut self, f: HarnessSpecFixture) -> HarnessResult<SpecId> {
        let id = f.id;
        self.store
            .seed_spec(NewSpec {
                id,
                project_id: f.project_id,
                name: f.name,
                needs_attention: f.needs_attention,
                attention_reason: f.attention_reason,
                created_at: f.created_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_spec: {e}")))?;
        Ok(id)
    }

    async fn seed_view_state(
        &mut self,
        aid: AccountId,
        pid: ProjectId,
        state: Value,
    ) -> HarnessResult<()> {
        self.store
            .write_view_state(aid, pid, state, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("view_state: {e}")))?;
        Ok(())
    }

    async fn list_projects(&mut self, aid: AccountId) -> HarnessResult<Vec<ProjectView>> {
        let out = run_cli(
            &self.binary,
            &[
                "project",
                "list",
                "--database-url",
                &self.db_url,
                "--account-id",
                &aid.to_string(),
            ],
        )
        .await?;
        parse_project_list(&out)
    }

    async fn switch_active_project(
        &mut self,
        aid: AccountId,
        pid: ProjectId,
    ) -> HarnessResult<SwitchProjectResponse> {
        let out = run_cli(
            &self.binary,
            &[
                "project",
                "switch",
                "--database-url",
                &self.db_url,
                "--account-id",
                &aid.to_string(),
                "--project-id",
                &pid.to_string(),
            ],
        )
        .await?;
        parse_switch_response(&out, pid)
    }

    async fn attention_spec(
        &mut self,
        aid: AccountId,
        pid: ProjectId,
        sid: SpecId,
    ) -> HarnessResult<AttentionSpecView> {
        let out = run_cli(
            &self.binary,
            &[
                "project",
                "attention-spec",
                "--database-url",
                &self.db_url,
                "--account-id",
                &aid.to_string(),
                "--project-id",
                &pid.to_string(),
                "--spec-id",
                &sid.to_string(),
            ],
        )
        .await?;
        parse_attention_spec(&out)
    }

    async fn project_scoped_views(&mut self, aid: AccountId) -> HarnessResult<ProjectScopedViews> {
        let out = run_cli(
            &self.binary,
            &[
                "project",
                "scoped-views",
                "--database-url",
                &self.db_url,
                "--account-id",
                &aid.to_string(),
            ],
        )
        .await?;
        parse_scoped_views(&out)
    }
}
