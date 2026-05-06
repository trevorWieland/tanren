//! `@cli` harness — shells out to the `tanren-cli` binary against a
//! per-scenario `SQLite` file.
//!
//! The harness owns the database file, applies migrations once at
//! construction, and reads recent events directly via its own
//! `Store` handle. Each sign-up / sign-in / accept-invitation step
//! spawns a `tanren-cli account ...` subprocess and parses the
//! `account_id=... session=...` line from stdout.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use regex::Regex;
use secrecy::ExposeSecret;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, AssetAction, MigrationConcern, MigrationConcernKind,
    SignInRequest, SignUpRequest, UpgradePreviewResponse,
};
use tanren_identity_policy::{AccountId, Identifier, OrgId};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::process::Command;
use uuid::Uuid;

use super::api::{code_to_reason, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession, UpgradeHarness,
};

/// `@cli` wire harness.
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

#[async_trait]
impl UpgradeHarness for CliHarness {
    async fn upgrade_preview(&mut self, root: &Path) -> HarnessResult<UpgradePreviewResponse> {
        self.run_upgrade_subcommand(root, false).await
    }

    async fn upgrade_apply(&mut self, root: &Path) -> HarnessResult<UpgradePreviewResponse> {
        self.run_upgrade_subcommand(root, true).await
    }
}

impl CliHarness {
    async fn run_upgrade_subcommand(
        &mut self,
        root: &Path,
        confirm: bool,
    ) -> HarnessResult<UpgradePreviewResponse> {
        let mut args = vec![
            "upgrade".to_owned(),
            "--root".to_owned(),
            root.to_string_lossy().to_string(),
        ];
        if confirm {
            args.push("--confirm".to_owned());
        }
        let output = Command::new(&self.binary)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli upgrade: {e}")))?;
        if !output.status.success() {
            return Err(translate_cli_error(&output.stderr));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_upgrade_response(&stdout)
    }
}

fn parse_upgrade_response(stdout: &str) -> HarnessResult<UpgradePreviewResponse> {
    let version_re =
        Regex::new(r"Upgrade (?:preview|applied): ([^ ]+) -> ([^ ]+)").expect("constant regex");
    let captures = version_re.captures(stdout).ok_or_else(|| {
        HarnessError::Transport(format!("could not parse upgrade output: {stdout}"))
    })?;
    let source = captures.get(1).map_or("", |m| m.as_str()).to_owned();
    let target = captures.get(2).map_or("", |m| m.as_str()).to_owned();

    let create_re = Regex::new(r"^\s{2}CREATE\s+(\S+)\s+\(([^)]+)\)$").expect("constant regex");
    let update_re =
        Regex::new(r"^\s{2}UPDATE\s+(\S+)\s+\(([^ ]+)\s*->\s*([^)]+)\)$").expect("constant regex");
    let remove_re = Regex::new(r"^\s{2}REMOVE\s+(\S+)\s+\(([^)]+)\)$").expect("constant regex");
    let preserve_re = Regex::new(r"^\s{2}PRESERVE\s+(\S+)\s+\(([^)]+)\)$").expect("constant regex");

    let concern_re = Regex::new(r"^\s{2}([a-z_]+):\s+(.+)$").expect("constant regex");

    let mut actions: Vec<AssetAction> = Vec::new();
    let mut concerns: Vec<MigrationConcern> = Vec::new();
    let mut preserved_user_paths: Vec<PathBuf> = Vec::new();
    let mut section: Option<&str> = None;

    for line in stdout.lines() {
        if line.starts_with("Upgrade preview:")
            || line.starts_with("Upgrade applied:")
            || line.is_empty()
        {
            continue;
        }
        if line == "Actions:" {
            section = Some("actions");
            continue;
        }
        if line == "Concerns:" {
            section = Some("concerns");
            continue;
        }
        if line == "Preserved user paths:" {
            section = Some("preserved");
            continue;
        }
        match section {
            Some("actions") => {
                if let Some(c) = create_re.captures(line) {
                    actions.push(AssetAction::Create {
                        path: PathBuf::from(c.get(1).map_or("", |m| m.as_str())),
                        hash: c.get(2).map_or("", |m| m.as_str()).to_owned(),
                    });
                } else if let Some(c) = update_re.captures(line) {
                    actions.push(AssetAction::Update {
                        path: PathBuf::from(c.get(1).map_or("", |m| m.as_str())),
                        old_hash: c.get(2).map_or("", |m| m.as_str()).to_owned(),
                        new_hash: c.get(3).map_or("", |m| m.as_str()).to_owned(),
                    });
                } else if let Some(c) = remove_re.captures(line) {
                    actions.push(AssetAction::Remove {
                        path: PathBuf::from(c.get(1).map_or("", |m| m.as_str())),
                        old_hash: c.get(2).map_or("", |m| m.as_str()).to_owned(),
                    });
                } else if let Some(c) = preserve_re.captures(line) {
                    actions.push(AssetAction::Preserve {
                        path: PathBuf::from(c.get(1).map_or("", |m| m.as_str())),
                        hash: c.get(2).map_or("", |m| m.as_str()).to_owned(),
                    });
                }
            }
            Some("concerns") => {
                if let Some(c) = concern_re.captures(line) {
                    let kind_str = c.get(1).map_or("", |m| m.as_str());
                    let detail = c.get(2).map_or("", |m| m.as_str()).to_owned();
                    let kind = match kind_str {
                        "hash_mismatch" => MigrationConcernKind::HashMismatch,
                        "removed_asset" => MigrationConcernKind::RemovedAsset,
                        "legacy_manifest" => MigrationConcernKind::LegacyManifest,
                        "user_asset_path_conflict" => MigrationConcernKind::UserAssetPathConflict,
                        _ => continue,
                    };
                    concerns.push(MigrationConcern {
                        kind,
                        path: PathBuf::new(),
                        detail,
                    });
                }
            }
            Some("preserved") => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    preserved_user_paths.push(PathBuf::from(trimmed));
                }
            }
            _ => {}
        }
    }

    Ok(UpgradePreviewResponse {
        source_version: source,
        target_version: target,
        actions,
        concerns,
        preserved_user_paths,
    })
}
