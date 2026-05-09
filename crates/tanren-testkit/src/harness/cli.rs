//! `@cli` harness — shells out to the `tanren-cli` binary against a
//! per-scenario `SQLite` file.
//!

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use regex::Regex;
use secrecy::ExposeSecret;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, MY_PERMISSIONS_DEFAULT_LIMIT, MyPermissionsPageMeta,
    MyPermissionsResponse, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, Identifier, OrgId};
use tanren_store::{
    AccountStore, EventEnvelope, NewInvitation, NewPermissionConstraint, NewPermissionGrant,
    PermissionGrantScope,
};
use tokio::process::Command;
use uuid::Uuid;

use super::api::{code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessPermissionGrantFixture, HarnessPermissionScope, HarnessPermissionsView, HarnessResult,
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
        let session_file = self.db_path.with_extension("session");
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
        // The CLI binary returns the AccountView reconstituted from
        // the row; re-decorate it with `org = Some(joined_org)` to
        // mirror the api/in-process surface where the account view
        // already carries the org id.
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

    async fn my_permissions(
        &mut self,
        _session_account_id: AccountId,
        requested_account_id: Option<AccountId>,
    ) -> HarnessResult<HarnessPermissionsView> {
        let mut args = vec![
            "account".to_owned(),
            "my-permissions".to_owned(),
            "--database-url".to_owned(),
            self.db_url.clone(),
        ];
        if let Some(target) = requested_account_id {
            args.push("--target-account-id".to_owned());
            args.push(target.to_string());
        }
        let session_file = self.db_path.with_extension("session");
        let output = Command::new(&self.binary)
            .args(args)
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
        let rendered = String::from_utf8_lossy(&output.stdout).to_string();
        let permissions = parse_permissions_output(&rendered)?;
        Ok(HarnessPermissionsView {
            response: permissions,
            rendered,
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

    async fn seed_permission_grant(
        &mut self,
        fixture: HarnessPermissionGrantFixture,
    ) -> HarnessResult<()> {
        let scope = match fixture.scope {
            HarnessPermissionScope::Organization(org_id) => {
                PermissionGrantScope::Organization(org_id)
            }
            HarnessPermissionScope::Project(project_id) => {
                PermissionGrantScope::Project(project_id)
            }
        };
        let grant = self
            .store
            .seed_permission_grant(NewPermissionGrant {
                account_id: fixture.account_id,
                scope,
                permission: fixture.permission,
                grant_source: fixture.grant_source,
                created_at: Utc::now(),
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_permission_grant: {e}")))?;
        if let Some(constraint) = fixture.policy_constraint {
            self.store
                .seed_permission_constraint(NewPermissionConstraint {
                    grant_id: grant.id,
                    reason: constraint.reason,
                    source: constraint.source,
                    created_at: Utc::now(),
                })
                .await
                .map_err(|e| HarnessError::Transport(format!("seed_permission_constraint: {e}")))?;
        }
        Ok(())
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
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
    let re = Regex::new(r"error:\s*([a-z_]+)\s*—\s*(.*)").expect("constant regex");
    if let Some(captures) = re.captures(&text) {
        let code = captures.get(1).map_or("", |m| m.as_str());
        let summary = captures.get(2).map_or("", |m| m.as_str()).trim().to_owned();
        if let Some(reason) = code_to_reason(code) {
            return HarnessError::Account(reason, summary);
        }
        return HarnessError::FailureCode {
            code: code.to_owned(),
            summary,
        };
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

fn parse_permissions_output(stdout: &str) -> HarnessResult<MyPermissionsResponse> {
    let mut organizations = Vec::new();
    let mut projects = Vec::new();
    let mut returned: u16 = 0;
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line == "permissions=none" {
            continue;
        }
        let scope = if line.contains("scope=organization") {
            "organization"
        } else if line.contains("scope=project") {
            "project"
        } else {
            continue;
        };
        let scope_id = capture(line, r"scope_id=([0-9a-fA-F-]+)")?;
        let permission = parse_permission_name(line)?;
        let effective_state = if line.contains("effective_state=Constrained") {
            tanren_identity_policy::PermissionEffectiveState::Constrained
        } else {
            tanren_identity_policy::PermissionEffectiveState::Granted
        };
        let grant_source = if line.contains("source=Direct") {
            tanren_identity_policy::PermissionGrantSource::Direct
        } else {
            let role_template = capture(line, r#"RoleTemplateName\("([^"]+)"\)"#)?;
            tanren_identity_policy::PermissionGrantSource::RoleTemplate {
                role_template: tanren_identity_policy::RoleTemplateName::new(role_template),
            }
        };
        let policy_constraint = if line.contains("constraint_reason=none") {
            None
        } else {
            let reason = capture(
                line,
                r#"constraint_reason=PolicyConstraintReason\("([^"]+)"\)"#,
            )?;
            let source = if line.contains("constraint_source=OrganizationPolicy") {
                tanren_identity_policy::PolicyConstraintSource::OrganizationPolicy
            } else {
                tanren_identity_policy::PolicyConstraintSource::ProjectPolicy
            };
            Some(tanren_contract::PermissionConstraintView {
                reason: tanren_identity_policy::PolicyConstraintReason::new(reason),
                source,
            })
        };
        let entry = tanren_contract::MyPermissionEntry {
            permission: tanren_identity_policy::PermissionName::new(permission),
            effective_state,
            grant_source,
            policy_constraint,
        };
        returned = returned.saturating_add(1);
        match scope {
            "organization" => organizations.push(tanren_contract::MyOrganizationPermissions {
                org_id: OrgId::from(
                    Uuid::parse_str(&scope_id)
                        .map_err(|e| HarnessError::Transport(format!("parse org scope id: {e}")))?,
                ),
                permissions: vec![entry],
            }),
            "project" => projects.push(tanren_contract::MyProjectPermissions {
                project_id: tanren_identity_policy::ProjectId::from(
                    Uuid::parse_str(&scope_id).map_err(|e| {
                        HarnessError::Transport(format!("parse project scope id: {e}"))
                    })?,
                ),
                permissions: vec![entry],
            }),
            _ => {}
        }
    }
    Ok(MyPermissionsResponse {
        page: MyPermissionsPageMeta {
            limit: MY_PERMISSIONS_DEFAULT_LIMIT,
            returned,
        },
        organizations,
        projects,
    })
}

fn capture(line: &str, pattern: &str) -> HarnessResult<String> {
    let re = Regex::new(pattern).expect("constant regex");
    let captures = re
        .captures(line)
        .ok_or_else(|| HarnessError::Transport(format!("parse permissions line: {line}")))?;
    Ok(captures.get(1).map_or("", |m| m.as_str()).to_owned())
}

fn parse_permission_name(line: &str) -> HarnessResult<String> {
    if let Ok(name) = capture(line, r#"permission=PermissionName\("([^"]+)"\)"#) {
        return Ok(name);
    }
    let raw = capture(line, r#"permission=("[^"]+"|\S+)"#)?;
    Ok(raw.trim_matches('"').to_owned())
}
