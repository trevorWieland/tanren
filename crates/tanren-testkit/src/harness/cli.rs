//! `@cli` harness — shells out to the `tanren-cli` binary against a
//! per-scenario `SQLite` file.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use secrecy::{ExposeSecret, SecretString};
use tanren_app_services::Store;
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, UserSettingKey, UserSettingValue,
};
use tanren_contract::{AcceptInvitationRequest, AccountView, SignInRequest, SignUpRequest};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::process::Command;

use super::cli_parse::{
    extract_session_token_from_stdout, locate_workspace_binary, parse_config_entry_stdout,
    parse_config_list_stdout, parse_credential_list_stdout, parse_credential_stdout,
    parse_joined_org, parse_session, translate_cli_error,
};
use super::shared::{scenario_db_path, sqlite_url};
use super::types::HarnessConfigEntry;
use super::{
    AccountHarness, HarnessAcceptance, HarnessCredential, HarnessError, HarnessInvitation,
    HarnessKind, HarnessResult, HarnessSession,
};

pub struct CliHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    db_url: String,
    binary: PathBuf,
    sessions: HashMap<AccountId, String>,
    session_dir: PathBuf,
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

        let session_dir = std::env::temp_dir().join(format!(
            "tanren-bdd-cli-sessions-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&session_dir)
            .map_err(|e| HarnessError::Transport(format!("create session dir: {e}")))?;

        Ok(Self {
            store,
            db_path,
            db_url,
            binary,
            sessions: HashMap::new(),
            session_dir,
        })
    }
}

impl Drop for CliHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.session_dir);
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
        let (account, has_token) = parse_session(&stdout, req.email.as_str(), &req.display_name)?;
        let session_token = extract_session_token_from_stdout(&stdout);
        self.sessions.insert(account.id, session_token);
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at: Utc::now() + Duration::days(30),
            has_token,
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
        let (account, has_token) = parse_session(&stdout, req.email.as_str(), "")?;
        let session_token = extract_session_token_from_stdout(&stdout);
        self.sessions.insert(account.id, session_token);
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
        let session_token = extract_session_token_from_stdout(&stdout);
        self.sessions.insert(account.id, session_token);
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

    async fn set_user_config(
        &mut self,
        account_id: AccountId,
        key: UserSettingKey,
        value: UserSettingValue,
    ) -> HarnessResult<HarnessConfigEntry> {
        let output = self
            .run_with_session(
                account_id,
                &[
                    "config",
                    "user",
                    "set",
                    "--database-url",
                    &self.db_url,
                    "--key",
                    &key.to_string(),
                    "--value",
                    value.as_str(),
                ],
            )
            .await?;
        parse_config_entry_stdout(&output)
    }

    async fn get_user_config(
        &mut self,
        account_id: AccountId,
        key: UserSettingKey,
    ) -> HarnessResult<Option<HarnessConfigEntry>> {
        let all = self.list_user_config(account_id).await?;
        Ok(all.into_iter().find(|e| e.key == key))
    }

    async fn list_user_config(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<Vec<HarnessConfigEntry>> {
        let output = self
            .run_with_session(
                account_id,
                &["config", "user", "list", "--database-url", &self.db_url],
            )
            .await?;
        parse_config_list_stdout(&output)
    }

    async fn attempt_get_other_user_config(
        &mut self,
        actor_account_id: AccountId,
        _target_account_id: AccountId,
        key: UserSettingKey,
    ) -> HarnessResult<Option<HarnessConfigEntry>> {
        self.get_user_config(actor_account_id, key).await
    }

    async fn create_credential(
        &mut self,
        account_id: AccountId,
        kind: CredentialKind,
        name: String,
        secret: SecretString,
    ) -> HarnessResult<HarnessCredential> {
        let output = self
            .run_with_session(
                account_id,
                &[
                    "credential",
                    "add",
                    "--database-url",
                    &self.db_url,
                    "--kind",
                    &kind.to_string(),
                    "--name",
                    &name,
                    "--value",
                    secret.expose_secret(),
                ],
            )
            .await?;
        parse_credential_stdout(&output)
    }

    async fn list_credentials(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<Vec<HarnessCredential>> {
        let output = self
            .run_with_session(
                account_id,
                &["credential", "list", "--database-url", &self.db_url],
            )
            .await?;
        parse_credential_list_stdout(&output)
    }

    async fn attempt_update_credential(
        &mut self,
        account_id: AccountId,
        credential_id: CredentialId,
        secret: SecretString,
    ) -> HarnessResult<HarnessCredential> {
        let output = self
            .run_with_session(
                account_id,
                &[
                    "credential",
                    "update",
                    "--database-url",
                    &self.db_url,
                    "--id",
                    &credential_id.to_string(),
                    "--value",
                    secret.expose_secret(),
                ],
            )
            .await?;
        parse_credential_stdout(&output)
    }

    async fn attempt_remove_credential(
        &mut self,
        account_id: AccountId,
        credential_id: CredentialId,
    ) -> HarnessResult<bool> {
        let output = self
            .run_with_session(
                account_id,
                &[
                    "credential",
                    "remove",
                    "--database-url",
                    &self.db_url,
                    "--id",
                    &credential_id.to_string(),
                ],
            )
            .await?;
        let text = String::from_utf8_lossy(&output.stdout);
        let re = regex::Regex::new(r"removed=(true|false)").expect("constant regex");
        let captures = re.captures(&text).ok_or_else(|| {
            HarnessError::Transport(format!("could not parse removed from stdout: {text}"))
        })?;
        let val = captures.get(1).map_or("false", |m| m.as_str());
        Ok(val == "true")
    }
}

impl CliHarness {
    async fn run_with_session(
        &self,
        account_id: AccountId,
        args: &[&str],
    ) -> HarnessResult<std::process::Output> {
        let token = self
            .sessions
            .get(&account_id)
            .ok_or_else(|| HarnessError::Transport("no session for account".to_owned()))?;
        let session_file = self.session_dir.join(format!("{account_id}.session"));
        std::fs::write(&session_file, token)
            .map_err(|e| HarnessError::Transport(format!("write session file: {e}")))?;
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
        Ok(output)
    }
}
