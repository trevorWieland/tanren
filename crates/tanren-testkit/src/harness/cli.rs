//! `@cli` harness — shells out to the `tanren-cli` binary against a
//! per-scenario `SQLite` file.
//!
//! The harness owns the database file, applies migrations once at
//! construction, and reads recent events directly via its own
//! `Store` handle. Each sign-up / sign-in / accept-invitation step
//! spawns a `tanren-cli account ...` subprocess and decodes the
//! typed JSON payload from stdout.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use regex::Regex;
use secrecy::ExposeSecret;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, DeploymentPostureReadModel,
    DeploymentPostureScope, SetDeploymentPostureRequest, SetDeploymentPostureResponse,
    SignInRequest, SignInResponse, SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::process::Command;

use super::api_support::{code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessPostureView, HarnessResult, HarnessSession, HarnessSupportedPosture,
};

/// `@cli` wire harness.
pub struct CliHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    session_path: PathBuf,
    db_url: String,
    binary: PathBuf,
}

impl std::fmt::Debug for CliHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CliHarness")
            .field("db_path", &self.db_path)
            .field("session_path", &self.session_path)
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
        let session_path = db_path.with_extension("session");
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
            session_path,
            db_url,
            binary,
        })
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.binary);
        command.env("TANREN_SESSION_FILE", &self.session_path);
        command
    }
}

impl Drop for CliHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.db_path);
        let _ = std::fs::remove_file(&self.session_path);
    }
}

#[async_trait]
impl AccountHarness for CliHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Cli
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let output = self
            .command()
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
        let response: SignUpResponse = parse_json_from_stdout(&output.stdout, "account create")?;
        Ok(HarnessSession {
            account_id: response.account.id,
            account: response.account,
            expires_at: response.session.expires_at,
            has_token: !response.session.token.expose_secret().is_empty(),
        })
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let output = self
            .command()
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
        let response: SignInResponse = parse_json_from_stdout(&output.stdout, "account sign-in")?;
        Ok(HarnessSession {
            account_id: response.account.id,
            account: response.account,
            expires_at: response.session.expires_at,
            has_token: !response.session.token.expose_secret().is_empty(),
        })
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let output = self
            .command()
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
        let response: AcceptInvitationResponse =
            parse_json_from_stdout(&output.stdout, "account create --invitation")?;
        Ok(HarnessAcceptance {
            session: HarnessSession {
                account_id: response.account.id,
                account: response.account,
                expires_at: response.session.expires_at,
                has_token: !response.session.token.expose_secret().is_empty(),
            },
            joined_org: response.joined_org,
        })
    }

    async fn list_supported_postures(&mut self) -> HarnessResult<Vec<HarnessSupportedPosture>> {
        let output = self
            .command()
            .args(["posture", "list"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli posture list: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let json: Value = parse_json_from_stdout(&output.stdout, "posture list")?;
        serde_json::from_value(json["supported"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode supported postures: {e}")))
    }

    async fn set_deployment_posture(
        &mut self,
        _actor: AccountId,
        request: SetDeploymentPostureRequest,
    ) -> HarnessResult<HarnessPostureView> {
        self.set_deployment_posture_raw(_actor, request.scope, request.posture.as_wire_value())
            .await
    }

    async fn set_deployment_posture_raw(
        &mut self,
        _actor: AccountId,
        scope: DeploymentPostureScope,
        posture_raw: &str,
    ) -> HarnessResult<HarnessPostureView> {
        let (scope_kind, scope_id) = scope_args(scope);
        let output = self
            .command()
            .args([
                "posture",
                "set",
                "--database-url",
                &self.db_url,
                "--scope-kind",
                scope_kind,
                "--scope-id",
                &scope_id,
                "--posture",
                posture_raw,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli posture set: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let json: Value = parse_json_from_stdout(&output.stdout, "posture set")?;
        let current: SetDeploymentPostureResponse = serde_json::from_value(json["current"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode posture response: {e}")))?;
        Ok(current.into())
    }

    async fn set_deployment_posture_raw_scope(
        &mut self,
        _actor: AccountId,
        scope_raw: Value,
        posture_raw: &str,
    ) -> HarnessResult<HarnessPostureView> {
        let (scope_kind, scope_id) = raw_scope_args(&scope_raw)?;
        let output = self
            .command()
            .args([
                "posture",
                "set",
                "--database-url",
                &self.db_url,
                "--scope-kind",
                &scope_kind,
                "--scope-id",
                &scope_id,
                "--posture",
                posture_raw,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli posture set: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let json: Value = parse_json_from_stdout(&output.stdout, "posture set")?;
        let current: SetDeploymentPostureResponse = serde_json::from_value(json["current"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode posture response: {e}")))?;
        Ok(current.into())
    }

    async fn get_deployment_posture(
        &mut self,
        _actor: AccountId,
        scope: DeploymentPostureScope,
    ) -> HarnessResult<Option<HarnessPostureView>> {
        let (scope_kind, scope_id) = scope_args(scope);
        let output = self
            .command()
            .args([
                "posture",
                "get",
                "--database-url",
                &self.db_url,
                "--scope-kind",
                scope_kind,
                "--scope-id",
                &scope_id,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli posture get: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let json: Value = parse_json_from_stdout(&output.stdout, "posture get")?;
        let current: Option<DeploymentPostureReadModel> =
            serde_json::from_value(json["current"].clone())
                .map_err(|e| HarnessError::Transport(format!("decode current posture: {e}")))?;
        Ok(current.map(Into::into))
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
        return HarnessError::FailureCode {
            code: code.to_owned(),
            summary,
        };
    }
    HarnessError::Transport(text.into_owned())
}

fn scope_args(scope: DeploymentPostureScope) -> (&'static str, String) {
    match scope {
        DeploymentPostureScope::Account { account_id } => ("account", account_id.to_string()),
        DeploymentPostureScope::Project { project_id } => ("project", project_id.to_string()),
        DeploymentPostureScope::Installation { installation_id } => {
            ("installation", installation_id.to_string())
        }
    }
}

fn raw_scope_args(scope_raw: &Value) -> HarnessResult<(String, String)> {
    let Value::Object(map) = scope_raw else {
        return Err(HarnessError::FailureCode {
            code: "validation_failed".to_owned(),
            summary: "deployment posture scope payload must be an object".to_owned(),
        });
    };
    let scope_kind =
        map.get("scope")
            .and_then(Value::as_str)
            .ok_or_else(|| HarnessError::FailureCode {
                code: "validation_failed".to_owned(),
                summary: "deployment posture scope payload is missing `scope`".to_owned(),
            })?;
    let scope_id_key = match scope_kind {
        "account" => "account_id",
        "project" => "project_id",
        "installation" => "installation_id",
        _ => {
            return Err(HarnessError::FailureCode {
                code: "validation_failed".to_owned(),
                summary: format!("unsupported deployment posture scope kind: {scope_kind}"),
            });
        }
    };
    let scope_id = map
        .get(scope_id_key)
        .and_then(Value::as_str)
        .ok_or_else(|| HarnessError::FailureCode {
            code: "validation_failed".to_owned(),
            summary: format!("deployment posture scope payload is missing `{scope_id_key}`"),
        })?;
    Ok((scope_kind.to_owned(), scope_id.to_owned()))
}

fn parse_json_from_stdout<T>(stdout: &[u8], operation: &str) -> HarnessResult<T>
where
    T: DeserializeOwned,
{
    let stdout = String::from_utf8_lossy(stdout);
    let candidate = stdout
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with('{'))
        .unwrap_or_default();
    serde_json::from_str(candidate).map_err(|e| {
        HarnessError::Transport(format!(
            "decode {operation} output as JSON: {e} (stdout={stdout})"
        ))
    })
}
