//! `@api` harness — spawns `tanren-api-app` on an ephemeral port and
//! drives it via `reqwest::Client` with `cookie_store(true)`.
//!
//! Uses one per-scenario `SQLite` file shared by the app store and
//! cookie-session store. Event assertions read through the harness's
//! own `Store` handle.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderValue;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, AccountView,
    CheckOrganizationPermissionResponse, CreateOrganizationResponse, ListOrganizationsResponse,
    SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, OrgId, OrganizationName, OrganizationPermission};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::common::{
    accept_invitation_body, code_to_reason, scenario_db_path, sign_in_body, sign_up_body,
    sqlite_url, wait_for_http_ready,
};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

/// `@api` wire harness.
pub struct ApiHarness {
    base_url: String,
    store: Arc<Store>,
    server: Option<JoinHandle<()>>,
    /// `SQLite` file path; deleted on drop.
    db_path: PathBuf,
    session_clients: HashMap<AccountId, Client>,
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
    /// per-scenario `SQLite` database file. Returns a harness ready to
    /// drive sign-up / sign-in / accept-invitation calls.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be connected /
    /// migrated, the listener cannot bind, or the api app cannot be
    /// constructed.
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
        wait_for_http_ready(&base_url, Duration::from_secs(2)).await?;

        Ok(Self {
            base_url,
            store,
            server: Some(server),
            db_path,
            session_clients: HashMap::new(),
        })
    }

    fn new_client() -> HarnessResult<Client> {
        Client::builder()
            .cookie_store(true)
            .timeout(super::HARNESS_DEFAULT_TIMEOUT)
            .build()
            .map_err(|e| HarnessError::Transport(format!("client build: {e}")))
    }

    fn session_client(&self, account_id: AccountId) -> HarnessResult<&Client> {
        self.session_clients
            .get(&account_id)
            .ok_or_else(super::auth_required_failure)
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
        let client = Self::new_client()?;
        let response = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /accounts: {e}")))?;
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
        let session = HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: cookies_set,
        };
        self.session_clients.insert(session.account_id, client);
        Ok(session)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = sign_in_body(&req);
        let url = format!("{}/sessions", self.base_url);
        let client = Self::new_client()?;
        let response = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /sessions: {e}")))?;
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
        let session = HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: cookies_set,
        };
        self.session_clients.insert(session.account_id, client);
        Ok(session)
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let body = accept_invitation_body(&req);
        let token = req.invitation_token.as_str().to_owned();
        let url = format!("{}/invitations/{token}/accept", self.base_url);
        let client = Self::new_client()?;
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
            .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))?;
        let joined_org = serde_json::from_value(json["joined_org"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
        let acceptance = HarnessAcceptance {
            session: HarnessSession {
                account_id: account.id,
                account,
                expires_at,
                has_token: cookies_set,
            },
            joined_org,
        };
        self.session_clients
            .insert(acceptance.session.account_id, client);
        Ok(acceptance)
    }

    async fn create_organization(
        &mut self,
        account_id: AccountId,
        name: OrganizationName,
    ) -> HarnessResult<CreateOrganizationResponse> {
        let body = serde_json::json!({ "name": name });
        let url = format!("{}/organizations", self.base_url);
        let client = if let Some(client) = self.session_clients.get(&account_id) {
            client.clone()
        } else {
            Self::new_client()?
        };
        let response = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /organizations: {e}")))?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode create_organization: {e}")))
    }

    async fn list_organizations(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ListOrganizationsResponse> {
        let url = format!("{}/organizations", self.base_url);
        let response = self
            .session_client(account_id)?
            .get(&url)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("GET /organizations: {e}")))?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode list_organizations: {e}")))
    }

    async fn check_organization_admin_permission(
        &mut self,
        account_id: AccountId,
        org_id: OrgId,
        permission: OrganizationPermission,
    ) -> HarnessResult<CheckOrganizationPermissionResponse> {
        let body = serde_json::json!({ "org_id": org_id, "permission": permission });
        let url = format!("{}/organizations/permissions/check", self.base_url);
        let response = self
            .session_client(account_id)?
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                HarnessError::Transport(format!("POST /organizations/permissions/check: {e}"))
            })?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let response: CheckOrganizationPermissionResponse =
            serde_json::from_value(json).map_err(|e| {
                HarnessError::Transport(format!("decode check_organization_permission: {e}"))
            })?;
        if response.allowed {
            Ok(response)
        } else {
            Err(HarnessError::Account(
                AccountFailureReason::PermissionDenied,
                AccountFailureReason::PermissionDenied.summary().to_owned(),
            ))
        }
    }

    async fn accept_invitations_concurrent(
        &mut self,
        requests: Vec<AcceptInvitationRequest>,
    ) -> Vec<HarnessResult<HarnessAcceptance>> {
        // Fan out via `tokio::spawn` so each acceptance issues its own
        // POST against the live api server in parallel. Each task gets
        // its own `reqwest::Client` (built fresh from a default
        // configuration) so cookie state from one task doesn't bleed
        // into another. The shared base URL is cheap to clone.
        //
        // Without this override, the trait's default impl would await
        // each request serially — defeating the @falsification @api
        // race scenario which is supposed to prove that
        // `consume_invitation` serializes concurrent acceptances at
        // the store layer (Codex P2 review on PR #133).
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

pub(crate) fn failure_from_body(json: &Value) -> HarnessError {
    let code = json
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("transport_error")
        .to_owned();
    let summary = json
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("unknown failure")
        .to_owned();
    if let Some(reason) = code_to_reason(&code) {
        HarnessError::Account(reason, summary)
    } else {
        HarnessError::Transport(format!("{code}: {summary}"))
    }
}
