//! `@api` harness — spawns `tanren-api-app` on an ephemeral port and
//! drives it via `reqwest::Client` with `cookie_store(true)`.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderValue;
use reqwest::Client;
use serde_json::Value;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, CreateOrganizationRequest, OrganizationAdminOperation,
    SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{AccountId, OrgId, OrgPermission};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::common::{
    accept_invitation_body, failure_from_body, scenario_db_path, sign_in_body, sign_up_body,
    sqlite_url,
};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessOrgSummary, HarnessOrganization, HarnessResult, HarnessSession,
};

pub struct ApiHarness {
    base_url: String,
    client: Client,
    store: Arc<Store>,
    server: Option<JoinHandle<()>>,
    db_path: PathBuf,
    current_account_id: Option<AccountId>,
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
            current_account_id: None,
        })
    }

    async fn post_json(&self, path: &str, body: &Value) -> HarnessResult<(Value, bool)> {
        let url = format!("{}{path}", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST {path}: {e}")))?;
        let status = response.status();
        let has_cookie = has_session_cookie(&response);
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        Ok((json, has_cookie))
    }

    async fn get_json(&self, path: &str) -> HarnessResult<Value> {
        let url = format!("{}{path}", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("GET {path}: {e}")))?;
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
        let (json, has_cookie) = self.post_json("/accounts", &sign_up_body(&req)).await?;
        let account = decode_account(&json)?;
        let expires_at = parse_expires_at(&json)?;
        self.current_account_id = Some(account.id);
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: has_cookie,
        })
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let (json, has_cookie) = self.post_json("/sessions", &sign_in_body(&req)).await?;
        let account = decode_account(&json)?;
        let expires_at = parse_expires_at(&json)?;
        self.current_account_id = Some(account.id);
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: has_cookie,
        })
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let token = req.invitation_token.as_str().to_owned();
        let path = format!("/invitations/{token}/accept");
        let (json, has_cookie) = self.post_json(&path, &accept_invitation_body(&req)).await?;
        let account = decode_account(&json)?;
        let expires_at = parse_expires_at(&json)?;
        let joined_org = serde_json::from_value(json["joined_org"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
        Ok(HarnessAcceptance {
            session: HarnessSession {
                account_id: account.id,
                account,
                expires_at,
                has_token: has_cookie,
            },
            joined_org,
        })
    }

    async fn accept_invitations_concurrent(
        &mut self,
        requests: Vec<AcceptInvitationRequest>,
    ) -> Vec<HarnessResult<HarnessAcceptance>> {
        let base_url = self.base_url.clone();
        let mut handles = Vec::with_capacity(requests.len());
        for req in requests {
            let url = format!(
                "{}/invitations/{}/accept",
                base_url,
                req.invitation_token.as_str()
            );
            let body = accept_invitation_body(&req);
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

    async fn create_organization(
        &mut self,
        req: CreateOrganizationRequest,
    ) -> HarnessResult<HarnessOrganization> {
        let body = serde_json::json!({ "name": req.name.as_str() });
        let json = self.post_json("/organizations", &body).await?.0;
        let org_id: OrgId = serde_json::from_value(json["organization"]["id"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode org id: {e}")))?;
        let name = json["organization"]["name"]
            .as_str()
            .unwrap_or("")
            .to_owned();
        let perms: Vec<OrgPermission> =
            serde_json::from_value(json["membership"]["permissions"].clone())
                .map_err(|e| HarnessError::Transport(format!("decode permissions: {e}")))?;
        Ok(HarnessOrganization {
            org_id,
            name,
            granted_permissions: perms,
            project_count: 0,
        })
    }

    async fn list_available_organizations(&mut self) -> HarnessResult<Vec<HarnessOrgSummary>> {
        let json = self.get_json("/account/organizations").await?;
        Ok(json["organizations"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        let id: OrgId = serde_json::from_value(v["id"].clone()).ok()?;
                        let name = v["name"].as_str().unwrap_or("").to_owned();
                        Some(HarnessOrgSummary { id, name })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn authorize_admin_operation(
        &mut self,
        org_id: OrgId,
        operation: OrganizationAdminOperation,
    ) -> HarnessResult<()> {
        let op_str = serde_json::to_value(operation)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let path = format!("/organizations/{org_id}/admin-operations/{op_str}/authorize");
        let response = self
            .client
            .post(format!("{}{path}", self.base_url))
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST authorize: {e}")))?;
        if response.status().is_success() {
            return Ok(());
        }
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        Err(failure_from_body(&json))
    }

    async fn probe_last_admin_protection(
        &mut self,
        org_id: OrgId,
        permission: OrgPermission,
    ) -> HarnessResult<()> {
        let account_id = super::require_account_id(self.current_account_id)?;
        super::probe_last_admin_via_store(self.store.as_ref(), org_id, account_id, permission).await
    }
}

fn has_session_cookie(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .any(|v| {
            v.to_str()
                .ok()
                .is_some_and(|s| s.starts_with("tanren_session="))
        })
}

fn decode_account(json: &Value) -> HarnessResult<AccountView> {
    serde_json::from_value(json["account"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode account: {e}")))
}

fn parse_expires_at(json: &Value) -> HarnessResult<chrono::DateTime<chrono::Utc>> {
    json["session"]["expires_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc))
        .ok_or_else(|| HarnessError::Transport("missing session.expires_at".to_owned()))
}
