//! `@cli` harness using subprocess calls to `tanren-cli`.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use regex::Regex;
use secrecy::ExposeSecret;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, ActiveProjectRequest, ActiveProjectView,
    ConnectProjectRepositoryRequest, ConnectProjectRepositoryResponse, ListVisibleProjectsRequest,
    ProjectCollectionView, ProjectCountsView, ProjectRepositoryView, ProjectSelectionView,
    ProjectView, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, Identifier, OrgId, ProjectId, RepositoryRef};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::process::Command;
use uuid::Uuid;

use super::api::{code_to_reason, project_code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession, ProjectHarness,
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
            .field("binary", &self.binary)
            .finish_non_exhaustive()
    }
}

impl CliHarness {
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = scenario_db_path("cli");
        let db_url = sqlite_url(&db_path);
        let store = Store::connect(&db_url)
            .await
            .map_err(|e| HarnessError::Transport(format!("connect store: {e}")))?;
        store
            .migrate()
            .await
            .map_err(|e| HarnessError::Transport(format!("migrate store: {e}")))?;
        let store = Arc::new(store);

        let binary = locate_workspace_binary("tanren-cli")?;

        Ok(Self {
            store,
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
        let output = Command::new(&self.binary)
            .args([
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
        parse_session(&stdout, req.email.as_str(), &req.display_name).map(|(account, has_token)| {
            HarnessSession {
                account_id: account.id,
                account,
                expires_at: Utc::now() + Duration::days(30),
                has_token,
            }
        })
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let output = Command::new(&self.binary)
            .args([
                "account",
                "sign-in",
                "--database-url",
                &self.db_url,
                "--identifier",
                req.email.as_str(),
                "--password",
                req.password.expose_secret(),
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
        parse_session(&stdout, req.email.as_str(), "").map(|(account, has_token)| HarnessSession {
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
        let output = Command::new(&self.binary)
            .args([
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
        let (account, has_token) = parse_session(&stdout, req.email.as_str(), &req.display_name)?;
        let joined_org = parse_joined_org(&stdout)?;
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
            .map_err(|e| HarnessError::Transport(format!("seed_invitation: {e}")))?;
        Ok(())
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }
}

#[async_trait]
impl ProjectHarness for CliHarness {
    async fn connect_project_repository(
        &mut self,
        req: ConnectProjectRepositoryRequest,
    ) -> HarnessResult<ConnectProjectRepositoryResponse> {
        let owning_account_id = req.owning_account_id;
        let output = Command::new(&self.binary)
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

pub(crate) fn locate_workspace_binary(name: &str) -> HarnessResult<PathBuf> {
    if let Ok(explicit) = std::env::var(format!(
        "TANREN_BIN_{}",
        name.replace('-', "_").to_uppercase()
    )) {
        let p = PathBuf::from(explicit);
        if p.exists() {
            return Ok(p);
        }
    }
    let exe = std::env::current_exe()
        .map_err(|e| HarnessError::Transport(format!("current exe: {e}")))?;
    let dir = exe
        .parent()
        .ok_or_else(|| HarnessError::Transport("current exe has no parent".to_owned()))?;
    let mut candidate = dir.join(name);
    if cfg!(windows) {
        candidate.set_extension("exe");
    }
    if candidate.exists() {
        return Ok(candidate);
    }
    let mut cursor = dir;
    while let Some(parent) = cursor.parent() {
        for profile in ["debug", "release"] {
            let mut probe = parent.join("target").join(profile).join(name);
            if cfg!(windows) {
                probe.set_extension("exe");
            }
            if probe.exists() {
                return Ok(probe);
            }
        }
        cursor = parent;
    }
    Err(HarnessError::Transport(format!(
        "binary `{name}` not found alongside test executable {} — run `cargo build --workspace`",
        exe.display()
    )))
}

fn translate_cli_error(stderr: &[u8]) -> HarnessError {
    let text = String::from_utf8_lossy(stderr);
    let re = Regex::new(r"error:\s*([a-z_]+)\s*—\s*(.*)").expect("constant regex");
    if let Some(captures) = re.captures(&text) {
        let code = captures.get(1).map_or("", |m| m.as_str());
        let summary = captures.get(2).map_or("", |m| m.as_str()).trim().to_owned();
        if let Some(reason) = code_to_reason(code) {
            return HarnessError::Account(reason, summary);
        }
        if let Some(reason) = project_code_to_reason(code) {
            return HarnessError::Project(reason, summary);
        }
    }
    HarnessError::Transport(text.into_owned())
}

fn parse_session(
    stdout: &str,
    email: &str,
    display_name: &str,
) -> HarnessResult<(AccountView, bool)> {
    let re = Regex::new(r"account_id=([0-9a-fA-F-]+)\s+session=([^\s]+)").expect("constant regex");
    let captures = re
        .captures(stdout)
        .ok_or_else(|| HarnessError::Transport(format!("could not parse cli stdout: {stdout}")))?;
    let id_raw = captures.get(1).map_or("", |m| m.as_str());
    let token = captures.get(2).map_or("", |m| m.as_str());
    let id = AccountId::from(
        Uuid::parse_str(id_raw)
            .map_err(|e| HarnessError::Transport(format!("parse account id: {e}")))?,
    );
    let identifier = Identifier::from_email(
        &tanren_identity_policy::Email::parse(email)
            .map_err(|e| HarnessError::Transport(format!("parse email: {e}")))?,
    );
    let account = AccountView {
        id,
        identifier,
        display_name: if display_name.is_empty() {
            String::new()
        } else {
            display_name.to_owned()
        },
        org: None,
    };
    Ok((account, !token.is_empty()))
}

fn parse_joined_org(stdout: &str) -> HarnessResult<OrgId> {
    let re = Regex::new(r"joined_org=([0-9a-fA-F-]+)").expect("constant regex");
    let captures = re.captures(stdout).ok_or_else(|| {
        HarnessError::Transport(format!(
            "could not parse joined_org from cli stdout: {stdout}"
        ))
    })?;
    let raw = captures.get(1).map_or("", |m| m.as_str());
    Ok(OrgId::from(Uuid::parse_str(raw).map_err(|e| {
        HarnessError::Transport(format!("parse org id: {e}"))
    })?))
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

    let id = ProjectId::from(
        Uuid::parse_str(project_id_raw)
            .map_err(|e| HarnessError::Transport(format!("parse project id: {e}")))?,
    );
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
        repository: ProjectRepositoryView { repository },
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
