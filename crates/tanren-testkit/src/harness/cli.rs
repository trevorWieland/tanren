//! `@cli` harness — shells out to the `tanren-cli` binary against a
//! per-scenario `SQLite` file.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use regex::Regex;
use secrecy::ExposeSecret;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, ListOrganizationProjectsResponse,
    OrganizationMembershipView, OrganizationSwitcher, SignInRequest, SignUpRequest,
    SwitchActiveOrganizationResponse,
};
use tanren_identity_policy::{AccountId, Identifier, OrgId};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation, NewOrganization, NewProject};
use tokio::process::Command;
use uuid::Uuid;

use super::api::{code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessOrganization, HarnessProject, HarnessResult, HarnessSession,
};

pub struct CliHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    db_url: String,
    binary: PathBuf,
    session_file: PathBuf,
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
        let session_file = db_path.with_extension("session");

        Ok(Self {
            store,
            db_path,
            db_url,
            binary,
            session_file,
        })
    }
}

impl Drop for CliHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.db_path);
        let _ = std::fs::remove_file(&self.session_file);
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
            .env("TANREN_SESSION_FILE", &self.session_file)
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
            .env("TANREN_SESSION_FILE", &self.session_file)
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
            .env("TANREN_SESSION_FILE", &self.session_file)
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

    async fn list_organizations(
        &mut self,
        _account_id: AccountId,
    ) -> HarnessResult<OrganizationSwitcher> {
        let output = Command::new(&self.binary)
            .args(["org", "list", "--database-url", &self.db_url])
            .env("TANREN_SESSION_FILE", &self.session_file)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        parse_org_list(&String::from_utf8_lossy(&output.stdout))
    }

    async fn switch_active_org(
        &mut self,
        _account_id: AccountId,
        org_id: OrgId,
    ) -> HarnessResult<SwitchActiveOrganizationResponse> {
        let output = Command::new(&self.binary)
            .args([
                "org",
                "switch",
                "--database-url",
                &self.db_url,
                "--org-id",
                &org_id.to_string(),
            ])
            .env("TANREN_SESSION_FILE", &self.session_file)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        parse_switch_result(&String::from_utf8_lossy(&output.stdout))
    }

    async fn list_active_org_projects(
        &mut self,
        _account_id: AccountId,
    ) -> HarnessResult<ListOrganizationProjectsResponse> {
        let output = Command::new(&self.binary)
            .args(["org", "projects", "--database-url", &self.db_url])
            .env("TANREN_SESSION_FILE", &self.session_file)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        parse_project_list(&String::from_utf8_lossy(&output.stdout))
    }

    async fn seed_organization(&mut self, fixture: HarnessOrganization) -> HarnessResult<()> {
        self.store
            .seed_organization(NewOrganization {
                id: fixture.org_id,
                name: fixture.org_name,
                created_at: Utc::now(),
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_organization: {e}")))?;
        Ok(())
    }

    async fn seed_membership(&mut self, account_id: AccountId, org_id: OrgId) -> HarnessResult<()> {
        self.store
            .seed_membership(account_id, org_id, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_membership: {e}")))?;
        Ok(())
    }

    async fn seed_project(&mut self, fixture: HarnessProject) -> HarnessResult<()> {
        self.store
            .seed_project(NewProject {
                id: fixture.project_id,
                org_id: fixture.org_id,
                name: fixture.project_name,
                created_at: Utc::now(),
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_project: {e}")))?;
        Ok(())
    }
}

pub(crate) fn locate_workspace_binary(name: &str) -> HarnessResult<PathBuf> {
    let env_key = format!("TANREN_BIN_{}", name.replace('-', "_").to_uppercase());
    if let Ok(explicit) = std::env::var(&env_key) {
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

fn parse_uuid(raw: &str) -> HarnessResult<Uuid> {
    Uuid::parse_str(raw).map_err(|e| HarnessError::Transport(format!("parse uuid: {e}")))
}

fn translate_cli_error(stderr: &[u8]) -> HarnessError {
    let text = String::from_utf8_lossy(stderr);
    let re = Regex::new(r"error:\s*([a-z_-]+)\s*—\s*(.*)").expect("constant regex");
    if let Some(captures) = re.captures(&text) {
        let code = captures.get(1).map_or("", |m| m.as_str());
        let summary = captures.get(2).map_or("", |m| m.as_str()).trim().to_owned();
        if let Some(reason) = code_to_reason(code) {
            return HarnessError::Account(reason, summary);
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
    let id = AccountId::from(parse_uuid(captures.get(1).map_or("", |m| m.as_str()))?);
    let token = captures.get(2).map_or("", |m| m.as_str());
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
    Ok(OrgId::from(parse_uuid(
        captures.get(1).map_or("", |m| m.as_str()),
    )?))
}

fn parse_org_list(stdout: &str) -> HarnessResult<OrganizationSwitcher> {
    if stdout.trim().starts_with("organizations: none") {
        return Ok(OrganizationSwitcher {
            memberships: Vec::new(),
            active_org: None,
        });
    }
    let re = Regex::new(r#"org_id=([0-9a-fA-F-]+)\s+name="([^"]*)"\s+active=(true|false)"#)
        .expect("constant regex");
    let mut memberships = Vec::new();
    let mut active_org = None;
    for cap in re.captures_iter(stdout) {
        let org_id = OrgId::from(parse_uuid(cap.get(1).map_or("", |m| m.as_str()))?);
        let org_name = cap.get(2).map_or("", |m| m.as_str()).to_owned();
        if cap.get(3).map_or("", |m| m.as_str()) == "true" {
            active_org = Some(org_id);
        }
        memberships.push(OrganizationMembershipView { org_id, org_name });
    }
    Ok(OrganizationSwitcher {
        memberships,
        active_org,
    })
}

fn parse_switch_result(stdout: &str) -> HarnessResult<SwitchActiveOrganizationResponse> {
    let re = Regex::new(r"active_org=([0-9a-fA-F-]+|none)").expect("constant regex");
    let captures = re.captures(stdout).ok_or_else(|| {
        HarnessError::Transport(format!("could not parse switch result: {stdout}"))
    })?;
    let raw = captures.get(1).map_or("", |m| m.as_str());
    let org = if raw == "none" {
        None
    } else {
        Some(OrgId::from(parse_uuid(raw)?))
    };
    Ok(SwitchActiveOrganizationResponse {
        account: AccountView {
            id: AccountId::from(Uuid::nil()),
            identifier: Identifier::from_email(
                &tanren_identity_policy::Email::parse("harness@cli")
                    .map_err(|e| HarnessError::Transport(format!("parse email: {e}")))?,
            ),
            display_name: String::new(),
            org,
        },
    })
}

fn parse_project_list(stdout: &str) -> HarnessResult<ListOrganizationProjectsResponse> {
    if stdout.trim().starts_with("projects: none") {
        return Ok(ListOrganizationProjectsResponse {
            projects: Vec::new(),
        });
    }
    let re = Regex::new(r#"project_id=([0-9a-fA-F-]+)\s+name="([^"]*)"\s+org_id=([0-9a-fA-F-]+)"#)
        .expect("constant regex");
    let mut projects = Vec::new();
    for cap in re.captures_iter(stdout) {
        let id = tanren_identity_policy::ProjectId::from(parse_uuid(
            cap.get(1).map_or("", |m| m.as_str()),
        )?);
        let name = cap.get(2).map_or("", |m| m.as_str()).to_owned();
        let org = OrgId::from(parse_uuid(cap.get(3).map_or("", |m| m.as_str()))?);
        projects.push(tanren_contract::ProjectView { id, name, org });
    }
    Ok(ListOrganizationProjectsResponse { projects })
}
