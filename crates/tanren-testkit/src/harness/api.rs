//! `@api` harness — spawns `tanren-api-app` on an ephemeral port and
//! drives it via `reqwest::Client` with `cookie_store(true)`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderValue;
use reqwest::Client;
use secrecy::SecretString;
use serde_json::Value;
use tanren_app_services::Store;
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, UserSettingKey, UserSettingValue,
};
use tanren_contract::{AcceptInvitationRequest, AccountView, SignInRequest, SignUpRequest};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::shared::{
    accept_invitation_body, decode_credential, extract_session_cookie, failure_from_body,
    run_concurrent_acceptances, scenario_db_path, sign_in_body, sign_up_body, sqlite_url,
};
use super::types::HarnessConfigEntry;
use super::{
    AccountHarness, HarnessAcceptance, HarnessCredential, HarnessError, HarnessInvitation,
    HarnessKind, HarnessResult, HarnessSession,
};

pub struct ApiHarness {
    base_url: String,
    client: Client,
    store: Arc<Store>,
    server: Option<JoinHandle<()>>,
    db_path: PathBuf,
    sessions: HashMap<AccountId, String>,
}

impl std::fmt::Debug for ApiHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiHarness")
            .field("base_url", &self.base_url)
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl ApiHarness {
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = scenario_db_path("api");
        let database_url = sqlite_url(&db_path);
        let store = Store::connect(&database_url)
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
        let base_url = format!("http://{local_addr}");

        let cors_origin = HeaderValue::from_str(&base_url)
            .map_err(|e| HarnessError::Transport(format!("cors header: {e}")))?;
        let app = tanren_api_app::build_app_with_store(
            store.clone(),
            &database_url,
            vec![cors_origin],
            false,
        )
        .await
        .map_err(|e| HarnessError::Transport(format!("build app: {e}")))?;

        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = Client::builder()
            .cookie_store(true)
            .timeout(super::HARNESS_DEFAULT_TIMEOUT)
            .build()
            .map_err(|e| HarnessError::Transport(format!("client build: {e}")))?;

        Ok(Self {
            base_url,
            client,
            store,
            server: Some(server),
            db_path,
            sessions: HashMap::new(),
        })
    }
}

impl Drop for ApiHarness {
    fn drop(&mut self) {
        if let Some(handle) = self.server.take() {
            handle.abort();
        }
        let _ = std::fs::remove_file(&self.db_path);
    }
}

#[async_trait]
impl AccountHarness for ApiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Api
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let body = sign_up_body(&req);
        let url = format!("{}/accounts", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /accounts: {e}")))?;
        let status = response.status();
        let session_cookie = extract_session_cookie(&response);
        let cookies_set = session_cookie.is_some();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let session = build_session(&json, cookies_set)?;
        if let Some(cookie) = session_cookie {
            self.sessions.insert(session.account_id, cookie);
        }
        Ok(session)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = sign_in_body(&req);
        let url = format!("{}/sessions", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /sessions: {e}")))?;
        let status = response.status();
        let session_cookie = extract_session_cookie(&response);
        let cookies_set = session_cookie.is_some();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let session = build_session(&json, cookies_set)?;
        if let Some(cookie) = session_cookie {
            self.sessions.insert(session.account_id, cookie);
        }
        Ok(session)
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let body = accept_invitation_body(&req);
        let token = req.invitation_token.as_str().to_owned();
        let url = format!("{}/invitations/{token}/accept", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                HarnessError::Transport(format!("POST /invitations/{{token}}/accept: {e}"))
            })?;
        let status = response.status();
        let session_cookie = extract_session_cookie(&response);
        let cookies_set = session_cookie.is_some();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let session = build_session(&json, cookies_set)?;
        let joined_org = serde_json::from_value(json["joined_org"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
        if let Some(cookie) = session_cookie {
            self.sessions.insert(session.account_id, cookie);
        }
        Ok(HarnessAcceptance {
            session,
            joined_org,
        })
    }

    async fn accept_invitations_concurrent(
        &mut self,
        requests: Vec<AcceptInvitationRequest>,
    ) -> Vec<HarnessResult<HarnessAcceptance>> {
        run_concurrent_acceptances(self.base_url.clone(), requests).await
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
        let body = serde_json::json!({"key": key.to_string(), "value": value.as_str()});
        let json = self
            .authenticated_post("/me/config", account_id, body)
            .await?;
        decode_config_entry(&json["entry"])
    }

    async fn get_user_config(
        &mut self,
        account_id: AccountId,
        key: UserSettingKey,
    ) -> HarnessResult<Option<HarnessConfigEntry>> {
        let path = format!("/me/config/{key}");
        let json = self.authenticated_get(&path, account_id).await?;
        let entry_val = &json["entry"];
        if entry_val.is_null() {
            return Ok(None);
        }
        Ok(Some(decode_config_entry(entry_val)?))
    }

    async fn list_user_config(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<Vec<HarnessConfigEntry>> {
        let json = self.authenticated_get("/me/config", account_id).await?;
        let entries = json["entries"]
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
        use secrecy::ExposeSecret;
        let body = serde_json::json!({
            "kind": kind.to_string(),
            "name": name,
            "value": secret.expose_secret(),
        });
        let json = self
            .authenticated_post("/me/credentials", account_id, body)
            .await?;
        decode_credential(&json["credential"])
    }

    async fn list_credentials(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<Vec<HarnessCredential>> {
        let json = self
            .authenticated_get("/me/credentials", account_id)
            .await?;
        let creds = json["credentials"]
            .as_array()
            .ok_or_else(|| HarnessError::Transport("credentials not an array".to_owned()))?;
        let mut out = Vec::with_capacity(creds.len());
        for c in creds {
            out.push(decode_credential(c)?);
        }
        Ok(out)
    }

    async fn attempt_update_credential(
        &mut self,
        account_id: AccountId,
        credential_id: CredentialId,
        secret: SecretString,
    ) -> HarnessResult<HarnessCredential> {
        use secrecy::ExposeSecret;
        let path = format!("/me/credentials/{credential_id}");
        let body = serde_json::json!({"value": secret.expose_secret()});
        let json = self.authenticated_patch(&path, account_id, body).await?;
        decode_credential(&json["credential"])
    }

    async fn attempt_remove_credential(
        &mut self,
        account_id: AccountId,
        credential_id: CredentialId,
    ) -> HarnessResult<bool> {
        let path = format!("/me/credentials/{credential_id}");
        let json = self.authenticated_delete(&path, account_id).await?;
        json["removed"]
            .as_bool()
            .ok_or_else(|| HarnessError::Transport("missing removed".to_owned()))
    }
}

fn build_session(json: &Value, cookies_set: bool) -> HarnessResult<HarnessSession> {
    let account: AccountView = serde_json::from_value(json["account"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
    let expires_at = json["session"]["expires_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc))
        .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
    Ok(HarnessSession {
        account_id: account.id,
        account,
        expires_at,
        has_token: cookies_set,
    })
}

fn decode_config_entry(val: &Value) -> HarnessResult<HarnessConfigEntry> {
    let key = serde_json::from_value(val["key"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode key: {e}")))?;
    let value = serde_json::from_value(val["value"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode value: {e}")))?;
    let updated_at = val["updated_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc))
        .ok_or_else(|| HarnessError::Transport("missing entry.updated_at".to_owned()))?;
    Ok(HarnessConfigEntry {
        key,
        value,
        updated_at,
    })
}

impl ApiHarness {
    fn session_cookie_for(&self, account_id: AccountId) -> HarnessResult<String> {
        self.sessions
            .get(&account_id)
            .cloned()
            .ok_or_else(|| HarnessError::Transport("no session for account".to_owned()))
    }

    async fn send_authenticated(
        &self,
        method: reqwest::Method,
        path: &str,
        account_id: AccountId,
        body: Option<Value>,
    ) -> HarnessResult<Value> {
        let cookie = self.session_cookie_for(account_id)?;
        let url = format!("{}{path}", self.base_url);
        let client = Client::builder()
            .timeout(super::HARNESS_DEFAULT_TIMEOUT)
            .build()
            .map_err(|e| HarnessError::Transport(format!("build client: {e}")))?;
        let mut req = client
            .request(method, &url)
            .header("Cookie", format!("tanren_session={cookie}"));
        if let Some(b) = body {
            req = req.json(&b);
        }
        let response = req
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("request {path}: {e}")))?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        Ok(json)
    }

    async fn authenticated_get(&self, path: &str, account_id: AccountId) -> HarnessResult<Value> {
        self.send_authenticated(reqwest::Method::GET, path, account_id, None)
            .await
    }

    async fn authenticated_post(
        &self,
        path: &str,
        account_id: AccountId,
        body: Value,
    ) -> HarnessResult<Value> {
        self.send_authenticated(reqwest::Method::POST, path, account_id, Some(body))
            .await
    }

    async fn authenticated_patch(
        &self,
        path: &str,
        account_id: AccountId,
        body: Value,
    ) -> HarnessResult<Value> {
        self.send_authenticated(reqwest::Method::PATCH, path, account_id, Some(body))
            .await
    }

    async fn authenticated_delete(
        &self,
        path: &str,
        account_id: AccountId,
    ) -> HarnessResult<Value> {
        self.send_authenticated(reqwest::Method::DELETE, path, account_id, None)
            .await
    }
}
