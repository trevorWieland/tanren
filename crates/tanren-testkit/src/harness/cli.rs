//! `@cli` harness — shells out to the `tanren-cli` binary against a
//! per-scenario `SQLite` file.
//!
//! The harness owns the database file, applies migrations once at
//! construction, and reads recent events directly via its own
//! `Store` handle. Each sign-up / sign-in / accept-invitation step
//! spawns a `tanren-cli account ...` subprocess and parses the
//! `account_id=... session=...` line from stdout.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use regex::Regex;
use secrecy::ExposeSecret;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, CreateOrganizationRequest, OrganizationAdminOperation,
    SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, Identifier, OrgId, OrgPermission};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::process::Command;
use uuid::Uuid;

use super::common::{code_to_org_reason, code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessOrgSummary, HarnessOrganization, HarnessResult, HarnessSession,
};

pub struct CliHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    db_url: String,
    binary: PathBuf,
    session_file: PathBuf,
    current_account_id: Option<AccountId>,
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

        let session_file = {
            let mut p = std::env::temp_dir();
            p.push(format!(
                "tanren-bdd-cli-session-{}-{}.txt",
                std::process::id(),
                Uuid::new_v4().simple()
            ));
            p
        };

        Ok(Self {
            store,
            db_path,
            db_url,
            binary,
            session_file,
            current_account_id: None,
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
            .env("TANREN_SESSION_FILE", &self.session_file)
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
            self.current_account_id = Some(account.id);
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
            .env("TANREN_SESSION_FILE", &self.session_file)
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
        parse_session(&stdout, req.email.as_str(), "").map(|(account, has_token)| {
            self.current_account_id = Some(account.id);
            HarnessSession {
                account_id: account.id,
                account,
                expires_at: Utc::now() + Duration::days(30),
                has_token,
            }
        })
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let output = Command::new(&self.binary)
            .env("TANREN_SESSION_FILE", &self.session_file)
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

    async fn create_organization(
        &mut self,
        req: CreateOrganizationRequest,
    ) -> HarnessResult<HarnessOrganization> {
        let output = Command::new(&self.binary)
            .env("TANREN_SESSION_FILE", &self.session_file)
            .args([
                "organization",
                "create",
                "--database-url",
                &self.db_url,
                "--name",
                req.name.as_str(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli org create: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_org_create_output(&stdout)
    }

    async fn list_available_organizations(&mut self) -> HarnessResult<Vec<HarnessOrgSummary>> {
        let output = Command::new(&self.binary)
            .env("TANREN_SESSION_FILE", &self.session_file)
            .args(["organization", "list", "--database-url", &self.db_url])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli org list: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parse_org_list_output(&stdout))
    }

    async fn authorize_admin_operation(
        &mut self,
        org_id: OrgId,
        operation: OrganizationAdminOperation,
    ) -> HarnessResult<()> {
        let op_str = operation_to_str(operation);
        let output = Command::new(&self.binary)
            .env("TANREN_SESSION_FILE", &self.session_file)
            .args([
                "organization",
                "authorize-admin-operation",
                "--database-url",
                &self.db_url,
                "--org-id",
                &org_id.to_string(),
                "--operation",
                op_str,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli org authorize: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        Ok(())
    }

    async fn probe_last_admin_protection(
        &mut self,
        org_id: OrgId,
        permission: OrgPermission,
    ) -> HarnessResult<()> {
        let account_id = super::require_account_id(self.current_account_id)?;
        super::probe_last_admin_via_store(self.store.as_ref(), org_id, account_id, permission).await
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
        if let Some(reason) = code_to_org_reason(code) {
            return HarnessError::Organization(reason, summary);
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

fn parse_org_create_output(stdout: &str) -> HarnessResult<HarnessOrganization> {
    let id_re = Regex::new(r"org_id=([0-9a-fA-F-]+)").expect("constant regex");
    let id_cap = id_re.captures(stdout).ok_or_else(|| {
        HarnessError::Transport(format!("parse org_id from cli stdout: {stdout}"))
    })?;
    let org_id = OrgId::from(
        Uuid::parse_str(id_cap.get(1).map_or("", |m| m.as_str()))
            .map_err(|e| HarnessError::Transport(format!("parse org id: {e}")))?,
    );
    let name_re = Regex::new(r"name=([^\s]+)").expect("constant regex");
    let name = name_re
        .captures(stdout)
        .and_then(|c| c.get(1))
        .map_or(String::new(), |m| m.as_str().to_owned());
    let count_re = Regex::new(r"project_count=(\d+)").expect("constant regex");
    let project_count: u64 = count_re
        .captures(stdout)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
        .unwrap_or(0);
    let perms_re = Regex::new(r"permissions=\[([^\]]*)\]").expect("constant regex");
    let granted_permissions =
        perms_re
            .captures(stdout)
            .and_then(|c| c.get(1))
            .map_or(Vec::new(), |m| {
                m.as_str()
                    .split(", ")
                    .filter_map(|s| str_to_permission(s.trim()))
                    .collect()
            });
    Ok(HarnessOrganization {
        org_id,
        name,
        granted_permissions,
        project_count,
    })
}

fn parse_org_list_output(stdout: &str) -> Vec<HarnessOrgSummary> {
    if stdout.contains("organizations: (none)") {
        return Vec::new();
    }
    let re = Regex::new(r"org_id=([0-9a-fA-F-]+)\s+name=([^\s]+)").expect("constant regex");
    re.captures_iter(stdout)
        .filter_map(|cap| {
            let id = Uuid::parse_str(cap.get(1)?.as_str()).ok()?;
            let name = cap.get(2)?.as_str().to_owned();
            Some(HarnessOrgSummary {
                id: OrgId::from(id),
                name,
            })
        })
        .collect()
}

fn operation_to_str(op: OrganizationAdminOperation) -> &'static str {
    match op {
        OrganizationAdminOperation::InviteMembers => "invite_members",
        OrganizationAdminOperation::ManageAccess => "manage_access",
        OrganizationAdminOperation::Configure => "configure",
        OrganizationAdminOperation::SetPolicy => "set_policy",
        OrganizationAdminOperation::Delete => "delete",
        _ => "unknown",
    }
}

fn str_to_permission(s: &str) -> Option<OrgPermission> {
    match s {
        "invite_members" => Some(OrgPermission::InviteMembers),
        "manage_access" => Some(OrgPermission::ManageAccess),
        "configure" => Some(OrgPermission::Configure),
        "set_policy" => Some(OrgPermission::SetPolicy),
        "delete" => Some(OrgPermission::Delete),
        _ => None,
    }
}
