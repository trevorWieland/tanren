//! `@api` harness — spawns `tanren-api-app` on an ephemeral port and
//! drives it via `reqwest::Client` with `cookie_store(true)`.
//!
//! The harness owns the `SQLite` database (a per-scenario file under
//! the OS temp directory). The same database is shared between (a)
//! the `Arc<Store>` injected into the api app for account-flow data
//! and (b) the tower-sessions sqlite-backed cookie store. Reading
//! recent events for the `Then a "..." event is recorded` step
//! goes through the harness's own `Store` handle (the api app's
//! `Arc<Store>` is a clone of the same `Store`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderValue;
use reqwest::Client;
use serde_json::Value;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, JoinOrganizationRequest, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, OrgId};
use tanren_store::{AccountStore, EventEnvelope};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::support::{
    failure_from_body, harness_to_new_invitation, scenario_db_path, sqlite_url, wait_for_http_ready,
};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessJoinResult,
    HarnessKind, HarnessResult, HarnessSession,
};

/// `@api` wire harness.
pub struct ApiHarness {
    base_url: String,
    client: Client,
    store: Arc<Store>,
    server: Option<JoinHandle<()>>,
    /// `SQLite` file path; deleted on drop.
    db_path: PathBuf,
    /// Per-actor tower-sessions cookie values, keyed by `AccountId`.
    /// Populated during `sign_up`/`sign_in` so `join_organization`
    /// can re-authenticate the correct actor in multi-actor scenarios.
    session_cookies: HashMap<AccountId, String>,
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
    /// Spawn a fresh `tanren-api-app` on an ephemeral port against a
    /// per-scenario `SQLite` database file.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be connected/migrated,
    /// the listener cannot bind, or the api app cannot be constructed.
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

        wait_for_http_ready(&client, &base_url).await;

        Ok(Self {
            base_url,
            client,
            store,
            server: Some(server),
            db_path,
            session_cookies: HashMap::new(),
        })
    }
}

impl Drop for ApiHarness {
    fn drop(&mut self) {
        if let Some(handle) = self.server.take() {
            handle.abort();
        }
        // Best-effort cleanup of the per-scenario DB file. Errors are
        // intentionally ignored — temp dir cleanup will catch any stragglers.
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
        let client = Client::builder()
            .cookie_store(false)
            .timeout(super::HARNESS_DEFAULT_TIMEOUT)
            .build()
            .map_err(|e| HarnessError::Transport(format!("build sign-up client: {e}")))?;
        let response = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /accounts: {e}")))?;
        let status = response.status();
        let cookie_value = extract_session_cookie(&response);
        let cookies_set = cookie_value.is_some();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let account: AccountView = serde_json::from_value(json["account"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
        let expires_at = json["session"]["expires_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
        if let Some(cv) = &cookie_value {
            self.session_cookies.insert(account.id, cv.clone());
        }
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: cookies_set,
        })
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = sign_in_body(&req);
        let url = format!("{}/sessions", self.base_url);
        let client = Client::builder()
            .cookie_store(false)
            .timeout(super::HARNESS_DEFAULT_TIMEOUT)
            .build()
            .map_err(|e| HarnessError::Transport(format!("build sign-in client: {e}")))?;
        let response = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /sessions: {e}")))?;
        let status = response.status();
        let cookie_value = extract_session_cookie(&response);
        let cookies_set = cookie_value.is_some();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let account: AccountView = serde_json::from_value(json["account"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
        let expires_at = json["session"]["expires_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
        if let Some(cv) = &cookie_value {
            self.session_cookies.insert(account.id, cv.clone());
        }
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: cookies_set,
        })
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
        let cookies_set = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .any(|v| {
                v.to_str()
                    .ok()
                    .is_some_and(|s| s.starts_with("tanren_session="))
            });
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let account: AccountView = serde_json::from_value(json["account"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
        let expires_at = json["session"]["expires_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
        let joined_org = serde_json::from_value(json["joined_org"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
        Ok(HarnessAcceptance {
            session: HarnessSession {
                account_id: account.id,
                account,
                expires_at,
                has_token: cookies_set,
            },
            joined_org,
        })
    }

    async fn accept_invitations_concurrent(
        &mut self,
        requests: Vec<AcceptInvitationRequest>,
    ) -> Vec<HarnessResult<HarnessAcceptance>> {
        // Fan out via `tokio::spawn` so each acceptance hits the live
        // api server in parallel with its own `reqwest::Client`.
        // Without this override the trait default awaits serially,
        // defeating the @falsification race scenario.
        let base_url = self.base_url.clone();
        let mut handles = Vec::with_capacity(requests.len());
        for req in requests {
            let url = format!(
                "{}/invitations/{}/accept",
                base_url,
                req.invitation_token.as_str()
            );
            let body = accept_invitation_body(&req);
            // Each task builds its own client. cookie_store is irrelevant
            // here — the race scenario doesn't reuse the session.
            let client = match Client::builder().build() {
                Ok(c) => c,
                Err(e) => {
                    handles.push(tokio::spawn(async move {
                        Err::<HarnessAcceptance, HarnessError>(HarnessError::Transport(format!(
                            "build client: {e}"
                        )))
                    }));
                    continue;
                }
            };
            handles.push(tokio::spawn(async move {
                let response = client.post(&url).json(&body).send().await.map_err(|e| {
                    HarnessError::Transport(format!("POST /invitations/{{token}}/accept: {e}"))
                })?;
                let status = response.status();
                let cookies_set = response
                    .headers()
                    .get_all(reqwest::header::SET_COOKIE)
                    .iter()
                    .any(|v| {
                        v.to_str()
                            .ok()
                            .is_some_and(|s| s.starts_with("tanren_session="))
                    });
                let json: Value = response
                    .json()
                    .await
                    .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
                if !status.is_success() {
                    return Err(failure_from_body(&json));
                }
                let account: AccountView = serde_json::from_value(json["account"].clone())
                    .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))?;
                let expires_at = json["session"]["expires_at"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .ok_or_else(|| {
                        HarnessError::Transport("missing session.expires_at".to_owned())
                    })?;
                let joined_org = serde_json::from_value(json["joined_org"].clone())
                    .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
                Ok(HarnessAcceptance {
                    session: HarnessSession {
                        account_id: account.id,
                        account,
                        expires_at,
                        has_token: cookies_set,
                    },
                    joined_org,
                })
            }));
        }
        let mut out = Vec::with_capacity(handles.len());
        for h in handles {
            out.push(match h.await {
                Ok(r) => r,
                Err(e) => Err(HarnessError::Transport(format!("join: {e}"))),
            });
        }
        out
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.store
            .seed_invitation(harness_to_new_invitation(fixture))
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_invitation: {e}")))?;
        Ok(())
    }

    async fn seed_membership(&mut self, account_id: AccountId, org_id: OrgId) -> HarnessResult<()> {
        let now = chrono::Utc::now();
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
        let token = req.invitation_token.as_str();
        let url = format!("{}/invitations/{token}/join", self.base_url);
        let cookie = self.session_cookies.get(&account_id).ok_or_else(|| {
            HarnessError::Transport(format!(
                "no session cookie for account {account_id:?} — actor must sign up or sign in first"
            ))
        })?;
        let client = Client::builder()
            .cookie_store(false)
            .timeout(super::HARNESS_DEFAULT_TIMEOUT)
            .build()
            .map_err(|e| HarnessError::Transport(format!("build join client: {e}")))?;
        let response = client
            .post(&url)
            .header("Cookie", format!("tanren_session={cookie}"))
            .send()
            .await
            .map_err(|e| {
                HarnessError::Transport(format!("POST /invitations/{{token}}/join: {e}"))
            })?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        decode_join_result(&json)
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }

    async fn expire_session(&mut self, account_id: AccountId) -> HarnessResult<()> {
        if let Some(cookie) = self.session_cookies.get_mut(&account_id) {
            cookie.clear();
            cookie.push_str("expired");
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
                harness_to_new_invitation(fixture),
                Some(raw_org_permissions),
            )
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_corrupted_invitation: {e}")))?;
        Ok(())
    }
}

fn sign_up_body(req: &SignUpRequest) -> Value {
    use secrecy::ExposeSecret;
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
        "display_name": req.display_name,
    })
}

fn sign_in_body(req: &SignInRequest) -> Value {
    use secrecy::ExposeSecret;
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
    })
}

fn accept_invitation_body(req: &AcceptInvitationRequest) -> Value {
    use secrecy::ExposeSecret;
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
        "display_name": req.display_name,
    })
}

fn extract_session_cookie(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .find_map(|v| {
            let s = v.to_str().ok()?;
            let prefix = "tanren_session=";
            let start = s.find(prefix)?;
            let rest = &s[start + prefix.len()..];
            Some(rest[..rest.find(';').unwrap_or(rest.len())].to_owned())
        })
}

fn decode_join_result(json: &Value) -> HarnessResult<HarnessJoinResult> {
    let v =
        |key: &str| -> HarnessResult<Value> { Ok(json.get(key).cloned().unwrap_or(Value::Null)) };
    Ok(HarnessJoinResult {
        joined_org: serde_json::from_value(v("joined_org")?)
            .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?,
        membership_permissions: serde_json::from_value(v("membership_permissions")?)
            .map_err(|e| HarnessError::Transport(format!("decode membership_permissions: {e}")))?,
        selectable_organizations: serde_json::from_value(v("selectable_organizations")?).map_err(
            |e| HarnessError::Transport(format!("decode selectable_organizations: {e}")),
        )?,
        project_access_grants: serde_json::from_value(v("project_access_grants")?)
            .map_err(|e| HarnessError::Transport(format!("decode project_access_grants: {e}")))?,
    })
}
