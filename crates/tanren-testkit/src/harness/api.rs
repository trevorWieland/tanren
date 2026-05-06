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

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderValue;
use chrono::Utc;
use reqwest::Client;
use serde_json::Value;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AttentionSpecView, ProjectScopedViews, ProjectView, SignInRequest,
    SignUpRequest, SwitchProjectResponse,
};
use tanren_identity_policy::{AccountId, ProjectId, SpecId};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation, NewProject, NewSpec, ProjectStore};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::api_support::{
    accept_invitation_body, extract_account_and_expiry, failure_from_body, has_session_cookie,
    scenario_db_path, session_from_parts, sign_in_body, sign_up_body, sqlite_url,
};
use super::project::{HarnessProjectFixture, HarnessSpecFixture, ProjectHarness};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

pub struct ApiHarness {
    base_url: String,
    client: Client,
    store: Arc<Store>,
    server: Option<JoinHandle<()>>,
    db_path: PathBuf,
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
            .map_err(|e| HarnessError::Transport(format!("connect: {e}")))?;
        store
            .migrate()
            .await
            .map_err(|e| HarnessError::Transport(format!("migrate: {e}")))?;
        let store = Arc::new(store);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| HarnessError::Transport(format!("bind: {e}")))?;
        let addr = listener
            .local_addr()
            .map_err(|e| HarnessError::Transport(format!("addr: {e}")))?;
        let base_url = format!("http://{addr}");
        let cors = HeaderValue::from_str(&base_url)
            .map_err(|e| HarnessError::Transport(format!("cors: {e}")))?;
        let app =
            tanren_api_app::build_app_with_store(store.clone(), &database_url, vec![cors], false)
                .await
                .map_err(|e| HarnessError::Transport(format!("app: {e}")))?;
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let client = Client::builder()
            .cookie_store(true)
            .timeout(super::HARNESS_DEFAULT_TIMEOUT)
            .build()
            .map_err(|e| HarnessError::Transport(format!("client: {e}")))?;
        Ok(Self {
            base_url,
            client,
            store,
            server: Some(server),
            db_path,
        })
    }

    async fn post_json(&self, path: &str, body: &Value) -> HarnessResult<(Value, bool)> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST {path}: {e}")))?;
        let st = resp.status();
        let cookies = has_session_cookie(resp.headers());
        let json: Value = resp
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode: {e}")))?;
        if !st.is_success() {
            return Err(failure_from_body(&json));
        }
        Ok((json, cookies))
    }

    async fn get_json_value(&self, path: &str) -> HarnessResult<Value> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("GET {path}: {e}")))?;
        let st = resp.status();
        let json: Value = resp
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode: {e}")))?;
        if !st.is_success() {
            return Err(failure_from_body(&json));
        }
        Ok(json)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> HarnessResult<T> {
        let json = self.get_json_value(path).await?;
        serde_json::from_value(json).map_err(|e| HarnessError::Transport(format!("decode: {e}")))
    }
}

impl Drop for ApiHarness {
    fn drop(&mut self) {
        if let Some(h) = self.server.take() {
            h.abort();
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
        let (json, cookies) = self.post_json("/accounts", &sign_up_body(&req)).await?;
        let (account, expires_at) = extract_account_and_expiry(&json)?;
        Ok(session_from_parts(account, expires_at, cookies))
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let (json, cookies) = self.post_json("/sessions", &sign_in_body(&req)).await?;
        let (account, expires_at) = extract_account_and_expiry(&json)?;
        Ok(session_from_parts(account, expires_at, cookies))
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let token = req.invitation_token.as_str().to_owned();
        let (json, cookies) = self
            .post_json(
                &format!("/invitations/{token}/accept"),
                &accept_invitation_body(&req),
            )
            .await?;
        let (account, expires_at) = extract_account_and_expiry(&json)?;
        let joined_org = serde_json::from_value(json["joined_org"].clone())
            .map_err(|e| HarnessError::Transport(format!("joined_org: {e}")))?;
        Ok(HarnessAcceptance {
            session: session_from_parts(account, expires_at, cookies),
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
                let resp = client
                    .post(&url)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| HarnessError::Transport(format!("POST: {e}")))?;
                let st = resp.status();
                let cookies = has_session_cookie(resp.headers());
                let json: Value = resp
                    .json()
                    .await
                    .map_err(|e| HarnessError::Transport(format!("decode: {e}")))?;
                if !st.is_success() {
                    return Err(failure_from_body(&json));
                }
                let (account, expires_at) = extract_account_and_expiry(&json)?;
                let joined_org = serde_json::from_value(json["joined_org"].clone())
                    .map_err(|e| HarnessError::Transport(format!("org: {e}")))?;
                Ok(HarnessAcceptance {
                    session: session_from_parts(account, expires_at, cookies),
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
            .map_err(|e| HarnessError::Transport(format!("seed: {e}")))?;
        Ok(())
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("events: {e}")))
    }
}

#[async_trait]
impl ProjectHarness for ApiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Api
    }

    async fn seed_project(&mut self, f: HarnessProjectFixture) -> HarnessResult<ProjectId> {
        let id = f.id;
        self.store
            .seed_project(NewProject {
                id,
                account_id: f.account_id,
                name: f.name,
                state: f.state,
                created_at: f.created_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_project: {e}")))?;
        Ok(id)
    }

    async fn seed_spec(&mut self, f: HarnessSpecFixture) -> HarnessResult<SpecId> {
        let id = f.id;
        self.store
            .seed_spec(NewSpec {
                id,
                project_id: f.project_id,
                name: f.name,
                needs_attention: f.needs_attention,
                attention_reason: f.attention_reason,
                created_at: f.created_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_spec: {e}")))?;
        Ok(id)
    }

    async fn seed_view_state(
        &mut self,
        aid: AccountId,
        pid: ProjectId,
        state: Value,
    ) -> HarnessResult<()> {
        self.store
            .write_view_state(aid, pid, state, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("view_state: {e}")))?;
        Ok(())
    }

    async fn list_projects(&mut self, _aid: AccountId) -> HarnessResult<Vec<ProjectView>> {
        self.get_json("/projects").await
    }

    async fn switch_active_project(
        &mut self,
        _aid: AccountId,
        pid: ProjectId,
    ) -> HarnessResult<SwitchProjectResponse> {
        let path = format!("/projects/{pid}/switch");
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST {path}: {e}")))?;
        let st = resp.status();
        let json: Value = resp
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode: {e}")))?;
        if !st.is_success() {
            return Err(failure_from_body(&json));
        }
        serde_json::from_value(json).map_err(|e| HarnessError::Transport(format!("switch: {e}")))
    }

    async fn attention_spec(
        &mut self,
        _aid: AccountId,
        pid: ProjectId,
        sid: SpecId,
    ) -> HarnessResult<AttentionSpecView> {
        self.get_json(&format!("/projects/{pid}/specs/{sid}/attention"))
            .await
    }

    async fn project_scoped_views(&mut self, _aid: AccountId) -> HarnessResult<ProjectScopedViews> {
        let j = self.get_json_value("/projects/active/views").await?;
        Ok(ProjectScopedViews {
            project_id: from_field(&j, "project_id")?,
            specs: from_field(&j, "specs")?,
            loops: from_field(&j, "loops")?,
            milestones: from_field(&j, "milestones")?,
        })
    }
}

fn from_field<T: serde::de::DeserializeOwned>(j: &Value, field: &str) -> HarnessResult<T> {
    serde_json::from_value(j[field].clone())
        .map_err(|e| HarnessError::Transport(format!("{field}: {e}")))
}
