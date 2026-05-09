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
    AcceptInvitationRequest, AccountView, CheckOrganizationPermissionResponse,
    CreateOrganizationResponse, ListOrganizationsResponse, OrganizationView, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::{AccountId, OrgId, OrganizationName, OrganizationPermission};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::process::Command;
use uuid::Uuid;

use super::cli_support::{
    locate_workspace_binary, parse_joined_org, parse_session, translate_cli_error,
};
use super::common::{scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

/// `@cli` wire harness.
pub struct CliHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    db_url: String,
    binary: PathBuf,
    session_files: HashMap<AccountId, PathBuf>,
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
            session_files: HashMap::new(),
        })
    }

    fn fresh_session_file() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "tanren-bdd-cli-session-{}-{}.txt",
            std::process::id(),
            Uuid::new_v4().simple()
        ));
        path
    }

    fn session_file_for(&self, account_id: AccountId) -> HarnessResult<&PathBuf> {
        self.session_files
            .get(&account_id)
            .ok_or_else(super::auth_required_failure)
    }

    fn permission_key(permission: OrganizationPermission) -> &'static str {
        match permission {
            OrganizationPermission::Invite => "invite",
            OrganizationPermission::ManageAccess => "manage_access",
            OrganizationPermission::Configure => "configure",
            OrganizationPermission::SetPolicy => "set_policy",
            OrganizationPermission::Delete => "delete",
        }
    }

    fn parse_permissions(raw: &str) -> HarnessResult<Vec<OrganizationPermission>> {
        let mut out = Vec::new();
        for piece in raw.split(',').filter(|s| !s.is_empty()) {
            let permission = match piece {
                "invite" => OrganizationPermission::Invite,
                "manage_access" => OrganizationPermission::ManageAccess,
                "configure" => OrganizationPermission::Configure,
                "set_policy" => OrganizationPermission::SetPolicy,
                "delete" => OrganizationPermission::Delete,
                other => {
                    return Err(HarnessError::Transport(format!(
                        "unknown permission key in cli output: {other}"
                    )));
                }
            };
            out.push(permission);
        }
        Ok(out)
    }
}

impl Drop for CliHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.db_path);
        for file in self.session_files.values() {
            let _ = std::fs::remove_file(file);
        }
    }
}

#[async_trait]
impl AccountHarness for CliHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Cli
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let session_file = Self::fresh_session_file();
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
            .env("TANREN_SESSION_FILE", &session_file)
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
            self.session_files.insert(account.id, session_file);
            HarnessSession {
                account_id: account.id,
                account,
                expires_at: Utc::now() + Duration::days(30),
                has_token,
            }
        })
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let session_file = Self::fresh_session_file();
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
            .env("TANREN_SESSION_FILE", &session_file)
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
            self.session_files.insert(account.id, session_file);
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
        let session_file = Self::fresh_session_file();
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
            .env("TANREN_SESSION_FILE", &session_file)
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
        // The CLI binary returns the AccountView reconstituted from
        // the row; re-decorate it with `org = Some(joined_org)` to
        // mirror the api/in-process surface where the account view
        // already carries the org id.
        let account = AccountView {
            org: Some(joined_org),
            ..account
        };
        self.session_files.insert(account.id, session_file);
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

    async fn create_organization(
        &mut self,
        account_id: AccountId,
        name: OrganizationName,
    ) -> HarnessResult<CreateOrganizationResponse> {
        let session_file = self.session_file_for(account_id)?;
        let output = Command::new(&self.binary)
            .args([
                "organization",
                "create",
                "--database-url",
                &self.db_url,
                "--account-id",
                &account_id.to_string(),
                "--name",
                name.as_str(),
            ])
            .env("TANREN_SESSION_FILE", session_file)
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
        let re = Regex::new(
            r"organization_id=([0-9a-fA-F-]+)\s+name=([^\s]+)\s+granted_permissions=([a-z_,]*)",
        )
        .expect("constant regex");
        let captures = re.captures(&stdout).ok_or_else(|| {
            HarnessError::Transport(format!("could not parse create-org cli stdout: {stdout}"))
        })?;
        let org_id_raw = captures.get(1).map_or("", |m| m.as_str());
        let granted_raw = captures.get(3).map_or("", |m| m.as_str());
        let org_id = OrgId::from(
            Uuid::parse_str(org_id_raw)
                .map_err(|e| HarnessError::Transport(format!("parse organization id: {e}")))?,
        );
        Ok(CreateOrganizationResponse {
            organization: OrganizationView { id: org_id, name },
            granted_permissions: Self::parse_permissions(granted_raw)?,
        })
    }

    async fn list_organizations(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ListOrganizationsResponse> {
        let session_file = self.session_file_for(account_id)?;
        let output = Command::new(&self.binary)
            .args([
                "organization",
                "list",
                "--database-url",
                &self.db_url,
                "--account-id",
                &account_id.to_string(),
            ])
            .env("TANREN_SESSION_FILE", session_file)
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
        let row_re = Regex::new(r"organization_id=([0-9a-fA-F-]+)\s+name=([^\s]+)").expect("regex");
        let mut organizations = Vec::new();
        for line in stdout.lines() {
            if line.starts_with("organizations=") {
                continue;
            }
            let Some(captures) = row_re.captures(line) else {
                continue;
            };
            let id_raw = captures.get(1).map_or("", |m| m.as_str());
            let name_raw = captures.get(2).map_or("", |m| m.as_str());
            let id = OrgId::from(
                Uuid::parse_str(id_raw)
                    .map_err(|e| HarnessError::Transport(format!("parse organization id: {e}")))?,
            );
            let name = OrganizationName::parse(name_raw).map_err(|e| {
                HarnessError::Transport(format!("parse organization name from cli output: {e}"))
            })?;
            organizations.push(OrganizationView { id, name });
        }
        Ok(ListOrganizationsResponse { organizations })
    }

    async fn check_organization_admin_permission(
        &mut self,
        account_id: AccountId,
        org_id: OrgId,
        permission: OrganizationPermission,
    ) -> HarnessResult<CheckOrganizationPermissionResponse> {
        let session_file = self.session_file_for(account_id)?;
        let output = Command::new(&self.binary)
            .args([
                "organization",
                "check-permission",
                "--database-url",
                &self.db_url,
                "--account-id",
                &account_id.to_string(),
                "--org-id",
                &org_id.to_string(),
                "--permission",
                Self::permission_key(permission),
            ])
            .env("TANREN_SESSION_FILE", session_file)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        Ok(CheckOrganizationPermissionResponse {
            account_id,
            org_id,
            permission,
            allowed: true,
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
