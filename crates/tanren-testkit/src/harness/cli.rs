//! `@cli` harness — shells out to the `tanren-cli` binary against a
//! per-scenario `SQLite` file.
//!
//! The harness owns the database file, applies migrations once at
//! construction, and reads recent events directly via its own
//! `Store` handle. Each sign-up / sign-in / accept-invitation step
//! spawns a `tanren-cli account ...` subprocess and parses the
//! `account_id=... session=...` line from stdout.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use regex::Regex;
use secrecy::ExposeSecret;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, JoinOrganizationRequest, OrgMembershipView,
    ProjectAccessGrant, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, Identifier, OrgId, OrgPermissions};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::process::Command;
use uuid::Uuid;

use super::support::{code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessJoinResult,
    HarnessKind, HarnessResult, HarnessSession,
};

/// `@cli` wire harness.
pub struct CliHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    db_url: String,
    binary: PathBuf,
    sessions: HashMap<AccountId, String>,
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
    /// Construct a fresh CLI harness. Connects + migrates a per-
    /// scenario `SQLite` database and locates the `tanren-cli` binary
    /// alongside the running BDD executable.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be initialized or the
    /// binary is missing from the expected target directory.
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
            sessions: HashMap::new(),
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
        let (account, has_token, raw_token) =
            parse_session_with_token(&stdout, req.email.as_str(), &req.display_name)?;
        let session = HarnessSession {
            account_id: account.id,
            account: account.clone(),
            expires_at: Utc::now() + Duration::days(30),
            has_token,
        };
        self.sessions.insert(account.id, raw_token);
        Ok(session)
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
        let (account, has_token, raw_token) =
            parse_session_with_token(&stdout, req.email.as_str(), "")?;
        let session = HarnessSession {
            account_id: account.id,
            account: account.clone(),
            expires_at: Utc::now() + Duration::days(30),
            has_token,
        };
        self.sessions.insert(account.id, raw_token);
        Ok(session)
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
        let (account, has_token, raw_token) =
            parse_session_with_token(&stdout, req.email.as_str(), &req.display_name)?;
        let joined_org = parse_joined_org(&stdout)?;
        let account = AccountView {
            org: Some(joined_org),
            ..account
        };
        let session = HarnessSession {
            account_id: account.id,
            account: account.clone(),
            expires_at: Utc::now() + Duration::days(30),
            has_token,
        };
        self.sessions.insert(account.id, raw_token);
        Ok(HarnessAcceptance {
            session,
            joined_org,
        })
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.store
            .seed_invitation(NewInvitation {
                token: fixture.token,
                inviting_org_id: fixture.inviting_org,
                expires_at: fixture.expires_at,
                target_identifier: fixture.target_identifier,
                org_permissions: fixture.org_permissions,
                revoked: fixture.revoked,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_invitation: {e}")))?;
        Ok(())
    }

    async fn seed_membership(&mut self, account_id: AccountId, org_id: OrgId) -> HarnessResult<()> {
        let now = Utc::now();
        self.store
            .insert_membership(account_id, org_id, now)
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_membership: {e}")))?;
        Ok(())
    }

    async fn join_organization(
        &mut self,
        account_id: AccountId,
        req: JoinOrganizationRequest,
    ) -> HarnessResult<HarnessJoinResult> {
        let raw_token = self
            .sessions
            .get(&account_id)
            .ok_or_else(|| {
                HarnessError::Transport(format!("no session stored for account {account_id}"))
            })?
            .clone();
        let session_dir =
            std::env::temp_dir().join(format!("tanren-cli-session-{}", Uuid::new_v4().simple()));
        std::fs::create_dir_all(&session_dir)
            .map_err(|e| HarnessError::Transport(format!("create session dir: {e}")))?;
        let session_file = session_dir.join("session");
        std::fs::write(&session_file, &raw_token)
            .map_err(|e| HarnessError::Transport(format!("write session file: {e}")))?;
        let output = Command::new(&self.binary)
            .args([
                "account",
                "join",
                "--database-url",
                &self.db_url,
                "--invitation",
                req.invitation_token.as_str(),
            ])
            .env("TANREN_SESSION_FILE", &session_file)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli join: {e}")))?;
        let _ = std::fs::remove_dir_all(&session_dir);
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let joined_org = parse_joined_org(&stdout)?;
        let memberships = self
            .store
            .list_memberships_for_account(account_id)
            .await
            .map_err(|e| HarnessError::Transport(format!("list_memberships: {e}")))?;
        let joined_membership = memberships
            .iter()
            .find(|m| m.org_id == joined_org)
            .ok_or_else(|| HarnessError::Transport("joined org membership not found".to_owned()))?;
        let membership_permissions = joined_membership
            .org_permissions
            .clone()
            .unwrap_or(OrgPermissions::member());
        let selectable_organizations = memberships
            .into_iter()
            .map(|m| OrgMembershipView {
                org_id: m.org_id,
                permissions: m.org_permissions.unwrap_or(OrgPermissions::member()),
            })
            .collect();
        Ok(HarnessJoinResult {
            joined_org,
            membership_permissions,
            selectable_organizations,
            project_access_grants: Vec::<ProjectAccessGrant>::new(),
        })
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }

    async fn expire_session(&mut self, account_id: AccountId) -> HarnessResult<()> {
        if let Some(token) = self.sessions.get_mut(&account_id) {
            token.clear();
            token.push_str("expired");
        }
        Ok(())
    }

    async fn seed_corrupted_invitation(
        &mut self,
        fixture: HarnessInvitation,
        raw_org_permissions: String,
    ) -> HarnessResult<()> {
        self.store
            .seed_invitation_raw_permissions(
                NewInvitation {
                    token: fixture.token,
                    inviting_org_id: fixture.inviting_org,
                    expires_at: fixture.expires_at,
                    target_identifier: fixture.target_identifier,
                    org_permissions: fixture.org_permissions,
                    revoked: fixture.revoked,
                },
                Some(raw_org_permissions),
            )
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_corrupted_invitation: {e}")))?;
        Ok(())
    }
}

/// Locate a workspace binary by name. The BDD runner is at
/// `target/<profile>/tanren-bdd-runner`; sibling binaries live in
/// the same directory.
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
    // Fallback: walk up to the workspace root and check
    // `target/{debug,release}/<bin>`.
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
    // CLI emits `error: <code> — <summary>` per
    // crates/tanren-cli-app/src/lib.rs::account_error.
    let re = Regex::new(r"error:\s*([a-z_]+)\s*—\s*(.*)").expect("constant regex");
    if let Some(captures) = re.captures(&text) {
        let code = captures.get(1).map_or("", |m| m.as_str());
        let summary = captures.get(2).map_or("", |m| m.as_str()).trim().to_owned();
        if let Some(reason) = code_to_reason(code) {
            return HarnessError::Account(reason, summary);
        }
    }
    HarnessError::Transport(text.into_owned())
}

fn parse_session_with_token(
    stdout: &str,
    email: &str,
    display_name: &str,
) -> HarnessResult<(AccountView, bool, String)> {
    let re = Regex::new(r"account_id=([0-9a-fA-F-]+)\s+session=([^\s]+)").expect("constant regex");
    let captures = re
        .captures(stdout)
        .ok_or_else(|| HarnessError::Transport(format!("could not parse cli stdout: {stdout}")))?;
    let id_raw = captures.get(1).map_or("", |m| m.as_str());
    let token = captures.get(2).map_or("", |m| m.as_str()).to_owned();
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
    Ok((account, !token.is_empty(), token))
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
