//! `@mcp` harness — spawns `tanren-mcp-app` on an ephemeral port and
//! drives the three account-flow tools through the rmcp
//! streamable-HTTP client.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use rmcp::RoleClient;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, ClientInfo, Content, RawContent};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use tanren_app_services::Store;
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, CredentialScope, UserSettingKey, UserSettingValue,
};
use tanren_contract::{AcceptInvitationRequest, AccountView, SignInRequest, SignUpRequest};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::shared::{failure_from_body, scenario_db_path, sqlite_url};
use super::{
    AccountHarness, HarnessAcceptance, HarnessConfigEntry, HarnessCredential, HarnessError,
    HarnessInvitation, HarnessKind, HarnessResult, HarnessSession,
};

const TEST_API_KEY: &str = "bdd-test-key";

/// `@mcp` wire harness.
pub struct McpHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    client: Option<RunningService<RoleClient, ClientInfo>>,
    server: Option<JoinHandle<()>>,
    sessions: HashMap<AccountId, String>,
}

impl std::fmt::Debug for McpHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpHarness")
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl McpHarness {
    /// Spawn an ephemeral `tanren-mcp-app` and connect a client to it.
    ///
    /// # Errors
    ///
    /// Returns an error if the database, listener, server, or rmcp
    /// client handshake fails.
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = scenario_db_path("mcp");
        let db_url = sqlite_url(&db_path);
        let store = Store::connect(&db_url)
            .await
            .map_err(|e| HarnessError::Transport(format!("connect store: {e}")))?;
        store
            .migrate()
            .await
            .map_err(|e| HarnessError::Transport(format!("migrate store: {e}")))?;
        let store = Arc::new(store);

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| HarnessError::Transport(format!("bind listener: {e}")))?;
        let local_addr = listener
            .local_addr()
            .map_err(|e| HarnessError::Transport(format!("local addr: {e}")))?;

        let (router, cancellation) = tanren_mcp_app::build_router_with_store(
            store.clone(),
            SecretString::from(TEST_API_KEY.to_owned()),
        );

        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move { cancellation.cancelled_owned().await })
                .await;
        });

        // Build the rmcp client transport with the bearer-token header.
        let config =
            StreamableHttpClientTransportConfig::with_uri(format!("http://{local_addr}/mcp"))
                .auth_header(TEST_API_KEY.to_owned());
        let transport = StreamableHttpClientTransport::with_client(reqwest::Client::new(), config);
        let client = ClientInfo::default()
            .serve(transport)
            .await
            .map_err(|e| HarnessError::Transport(format!("rmcp serve: {e}")))?;

        Ok(Self {
            store,
            db_path,
            client: Some(client),
            server: Some(server),
            sessions: HashMap::new(),
        })
    }

    async fn call_tool(&mut self, name: &'static str, body: Value) -> HarnessResult<Value> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| HarnessError::Transport("rmcp client gone".to_owned()))?;
        let args: serde_json::Map<String, Value> = match body {
            Value::Object(map) => map,
            other => {
                return Err(HarnessError::Transport(format!(
                    "tool args must be a JSON object, got {other}"
                )));
            }
        };
        let result: CallToolResult = client
            .call_tool(CallToolRequestParams::new(name).with_arguments(args))
            .await
            .map_err(|e| HarnessError::Transport(format!("call_tool {name}: {e}")))?;
        let text = first_text(&result.content).ok_or_else(|| {
            HarnessError::Transport(format!("tool {name} returned no text content"))
        })?;
        let payload: Value = serde_json::from_str(&text)
            .map_err(|e| HarnessError::Transport(format!("decode tool result: {e}")))?;
        if result.is_error == Some(true) {
            return Err(failure_from_body(&payload));
        }
        Ok(payload)
    }
}

impl Drop for McpHarness {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            drop(client);
        }
        if let Some(handle) = self.server.take() {
            handle.abort();
        }
        let _ = std::fs::remove_file(&self.db_path);
    }
}

#[async_trait]
impl AccountHarness for McpHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Mcp
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let body = serde_json::json!({
            "email": req.email.as_str(),
            "password": req.password.expose_secret(),
            "display_name": req.display_name,
        });
        let payload = self.call_tool("account.create", body).await?;
        let session = decode_session(&payload)?;
        if let Some(token) = extract_session_token(&payload) {
            self.sessions.insert(session.account_id, token);
        }
        Ok(session)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = serde_json::json!({
            "email": req.email.as_str(),
            "password": req.password.expose_secret(),
        });
        let payload = self.call_tool("account.sign_in", body).await?;
        let session = decode_session(&payload)?;
        if let Some(token) = extract_session_token(&payload) {
            self.sessions.insert(session.account_id, token);
        }
        Ok(session)
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let body = serde_json::json!({
            "invitation_token": req.invitation_token.as_str(),
            "email": req.email.as_str(),
            "password": req.password.expose_secret(),
            "display_name": req.display_name,
        });
        let payload = self.call_tool("account.accept_invitation", body).await?;
        let session = decode_session(&payload)?;
        if let Some(token) = extract_session_token(&payload) {
            self.sessions.insert(session.account_id, token);
        }
        let joined_org = serde_json::from_value(payload["joined_org"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
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
        let token = self.session_token_for(account_id)?;
        let body = serde_json::json!({
            "session_token": token,
            "key": key.to_string(),
            "value": value.as_str(),
        });
        let payload = self.call_tool("user_config.set", body).await?;
        decode_config_entry(&payload["entry"])
    }

    async fn get_user_config(
        &mut self,
        account_id: AccountId,
        key: UserSettingKey,
    ) -> HarnessResult<Option<HarnessConfigEntry>> {
        let token = self.session_token_for(account_id)?;
        let body = serde_json::json!({
            "session_token": token,
            "key": key.to_string(),
        });
        let payload = self.call_tool("user_config.get", body).await?;
        let entry_val = &payload["entry"];
        if entry_val.is_null() {
            return Ok(None);
        }
        Ok(Some(decode_config_entry(entry_val)?))
    }

    async fn list_user_config(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<Vec<HarnessConfigEntry>> {
        let token = self.session_token_for(account_id)?;
        let body = serde_json::json!({"session_token": token});
        let payload = self.call_tool("user_config.list", body).await?;
        let entries = payload["entries"]
            .as_array()
            .ok_or_else(|| HarnessError::Transport("entries not an array".to_owned()))?;
        let mut out = Vec::with_capacity(entries.len());
        for e in entries {
            out.push(decode_config_entry(e)?);
        }
        Ok(out)
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
        let token = self.session_token_for(account_id)?;
        let body = serde_json::json!({
            "session_token": token,
            "kind": kind.to_string(),
            "name": name,
            "value": secret.expose_secret(),
        });
        let payload = self.call_tool("credential.add", body).await?;
        decode_harness_credential(&payload["credential"])
    }

    async fn list_credentials(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<Vec<HarnessCredential>> {
        let token = self.session_token_for(account_id)?;
        let body = serde_json::json!({"session_token": token});
        let payload = self.call_tool("credential.list", body).await?;
        let creds = payload["credentials"]
            .as_array()
            .ok_or_else(|| HarnessError::Transport("credentials not an array".to_owned()))?;
        let mut out = Vec::with_capacity(creds.len());
        for c in creds {
            out.push(decode_harness_credential(c)?);
        }
        Ok(out)
    }

    async fn attempt_update_credential(
        &mut self,
        account_id: AccountId,
        credential_id: CredentialId,
        secret: SecretString,
    ) -> HarnessResult<HarnessCredential> {
        let token = self.session_token_for(account_id)?;
        let body = serde_json::json!({
            "session_token": token,
            "id": credential_id.to_string(),
            "value": secret.expose_secret(),
        });
        let payload = self.call_tool("credential.update", body).await?;
        decode_harness_credential(&payload["credential"])
    }

    async fn attempt_remove_credential(
        &mut self,
        account_id: AccountId,
        credential_id: CredentialId,
    ) -> HarnessResult<bool> {
        let token = self.session_token_for(account_id)?;
        let body = serde_json::json!({
            "session_token": token,
            "id": credential_id.to_string(),
        });
        let payload = self.call_tool("credential.remove", body).await?;
        payload["removed"]
            .as_bool()
            .ok_or_else(|| HarnessError::Transport("missing removed".to_owned()))
    }
}

fn first_text(content: &[Content]) -> Option<String> {
    for item in content {
        if let RawContent::Text(text) = &item.raw {
            return Some(text.text.clone());
        }
    }
    None
}

fn decode_session(payload: &Value) -> HarnessResult<HarnessSession> {
    let account: AccountView = serde_json::from_value(payload["account"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
    let expires_at = payload["session"]["expires_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc))
        .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
    let token_present = payload["session"]["token"]
        .as_str()
        .is_some_and(|s| !s.is_empty());
    Ok(HarnessSession {
        account_id: account.id,
        account,
        expires_at,
        has_token: token_present,
    })
}

impl McpHarness {
    fn session_token_for(&self, account_id: AccountId) -> HarnessResult<String> {
        self.sessions
            .get(&account_id)
            .cloned()
            .ok_or_else(|| HarnessError::Transport("no session for account".to_owned()))
    }
}

fn extract_session_token(payload: &Value) -> Option<String> {
    payload["session"]["token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn decode_config_entry(val: &Value) -> HarnessResult<HarnessConfigEntry> {
    let key: UserSettingKey = serde_json::from_value(val["key"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode config key: {e}")))?;
    let value: UserSettingValue = serde_json::from_value(val["value"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode config value: {e}")))?;
    let updated_at = val["updated_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc))
        .ok_or_else(|| HarnessError::Transport("missing updated_at".to_owned()))?;
    Ok(HarnessConfigEntry {
        key,
        value,
        updated_at,
    })
}

fn decode_harness_credential(val: &Value) -> HarnessResult<HarnessCredential> {
    let id: CredentialId = serde_json::from_value(val["id"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode cred id: {e}")))?;
    let name = val["name"]
        .as_str()
        .ok_or_else(|| HarnessError::Transport("missing cred name".to_owned()))?
        .to_owned();
    let kind: CredentialKind = serde_json::from_value(val["kind"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode cred kind: {e}")))?;
    let scope: CredentialScope = serde_json::from_value(val["scope"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode cred scope: {e}")))?;
    let present = val["present"]
        .as_bool()
        .ok_or_else(|| HarnessError::Transport("missing cred present".to_owned()))?;
    Ok(HarnessCredential {
        id,
        name,
        kind,
        scope,
        present,
    })
}
