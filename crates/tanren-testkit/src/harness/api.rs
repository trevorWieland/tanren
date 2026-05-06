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
use reqwest::Client;
use serde_json::Value;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, AccountView, ActiveProjectView,
    ConnectProjectRequest, CreateProjectRequest, ProjectFailureReason, ProjectView, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::FixtureSourceControlProvider;

use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

/// `@api` wire harness.
pub struct ApiHarness {
    base_url: String,
    client: Client,
    store: Arc<Store>,
    provider: FixtureSourceControlProvider,
    server: Option<JoinHandle<()>>,
    /// `SQLite` file path; deleted on drop.
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
    /// Spawn a fresh `tanren-api-app` on an ephemeral port against a
    /// per-scenario `SQLite` database file.
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
        let provider = FixtureSourceControlProvider::new();

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| HarnessError::Transport(format!("bind listener: {e}")))?;
        let local_addr = listener
            .local_addr()
            .map_err(|e| HarnessError::Transport(format!("local addr: {e}")))?;
        let base_url = format!("http://{local_addr}");

        let cors_origin = HeaderValue::from_str(&base_url)
            .map_err(|e| HarnessError::Transport(format!("cors header: {e}")))?;
        let provider_for_server: Arc<dyn tanren_app_services::SourceControlProvider> =
            Arc::new(provider.clone());
        let app = tanren_api_app::build_app_with_store(
            store.clone(),
            &database_url,
            vec![cors_origin],
            false,
            Some(provider_for_server),
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
            provider,
            server: Some(server),
            db_path,
        })
    }

    async fn post_json(
        &self,
        path: &str,
        body: &Value,
    ) -> HarnessResult<(reqwest::StatusCode, Value, bool)> {
        let url = format!("{}{path}", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST {path}: {e}")))?;
        let status = response.status();
        let cookies = has_session_cookie(response.headers());
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        Ok((status, json, cookies))
    }

    async fn get_json(&self, path: &str) -> HarnessResult<(reqwest::StatusCode, Value)> {
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
        Ok((status, json))
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

fn has_session_cookie(headers: &reqwest::header::HeaderMap) -> bool {
    headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .any(|v| {
            v.to_str()
                .ok()
                .is_some_and(|s| s.starts_with("tanren_session="))
        })
}

fn parse_session(json: &Value, cookies_set: bool) -> HarnessResult<HarnessSession> {
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

fn parse_acceptance(json: &Value, cookies_set: bool) -> HarnessResult<HarnessAcceptance> {
    let session = parse_session(json, cookies_set)?;
    let joined_org = serde_json::from_value(json["joined_org"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
    Ok(HarnessAcceptance {
        session,
        joined_org,
    })
}

#[async_trait]
impl AccountHarness for ApiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Api
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let body = sign_up_body(&req);
        let (status, json, cookies) = self.post_json("/accounts", &body).await?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        parse_session(&json, cookies)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let body = sign_in_body(&req);
        let (status, json, cookies) = self.post_json("/sessions", &body).await?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        parse_session(&json, cookies)
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let body = accept_invitation_body(&req);
        let token = req.invitation_token.as_str().to_owned();
        let path = format!("/invitations/{token}/accept");
        let (status, json, cookies) = self.post_json(&path, &body).await?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        parse_acceptance(&json, cookies)
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
                let cookies = has_session_cookie(response.headers());
                let json: Value = response
                    .json()
                    .await
                    .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
                if !status.is_success() {
                    return Err(failure_from_body(&json));
                }
                parse_acceptance(&json, cookies)
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

    async fn connect_project(
        &mut self,
        _account_id: AccountId,
        request: ConnectProjectRequest,
    ) -> HarnessResult<ProjectView> {
        let body = serde_json::json!({
            "name": request.name,
            "repository_url": request.repository_url,
            "org": request.org,
        });
        let (status, json, _) = self.post_json("/projects/connect", &body).await?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode project: {e}")))
    }

    async fn create_project(
        &mut self,
        _account_id: AccountId,
        request: CreateProjectRequest,
    ) -> HarnessResult<ProjectView> {
        let body = serde_json::json!({
            "name": request.name,
            "provider_host": request.provider_host,
            "org": request.org,
        });
        let (status, json, _) = self.post_json("/projects/create", &body).await?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode project: {e}")))
    }

    async fn active_project(
        &mut self,
        _account_id: AccountId,
    ) -> HarnessResult<Option<ActiveProjectView>> {
        let (status, json) = self.get_json("/projects/active").await?;
        if status == axum::http::StatusCode::NO_CONTENT {
            return Ok(None);
        }
        serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode active: {e}")))
    }

    async fn seed_accessible_repository(&mut self, host: &str, url: &str) -> HarnessResult<()> {
        self.provider = self
            .provider
            .clone()
            .with_accessible_host(host)
            .with_existing_repository(url);
        Ok(())
    }

    async fn seed_inaccessible_host(&mut self, _host: &str) -> HarnessResult<()> {
        Ok(())
    }

    async fn seed_inaccessible_repository(&mut self, url: &str) -> HarnessResult<()> {
        self.provider = self.provider.clone().with_existing_repository(url);
        Ok(())
    }

    async fn seed_accessible_host(&mut self, host: &str) -> HarnessResult<()> {
        self.provider = self.provider.clone().with_accessible_host(host);
        Ok(())
    }
}

pub(crate) fn scenario_db_path(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "tanren-bdd-{prefix}-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    p
}

pub(crate) fn sqlite_url(path: &std::path::Path) -> String {
    format!("sqlite://{}?mode=rwc", path.display())
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
    } else if let Some(reason) = code_to_project_reason(&code) {
        HarnessError::Project(reason, summary)
    } else {
        HarnessError::Transport(format!("{code}: {summary}"))
    }
}

pub(crate) fn code_to_reason(code: &str) -> Option<AccountFailureReason> {
    Some(match code {
        "duplicate_identifier" => AccountFailureReason::DuplicateIdentifier,
        "invalid_credential" => AccountFailureReason::InvalidCredential,
        "validation_failed" => AccountFailureReason::ValidationFailed,
        "invitation_not_found" => AccountFailureReason::InvitationNotFound,
        "invitation_expired" => AccountFailureReason::InvitationExpired,
        "invitation_already_consumed" => AccountFailureReason::InvitationAlreadyConsumed,
        _ => return None,
    })
}

pub(crate) fn code_to_project_reason(code: &str) -> Option<ProjectFailureReason> {
    Some(match code {
        "access_denied" => ProjectFailureReason::AccessDenied,
        "duplicate_repository" => ProjectFailureReason::DuplicateRepository,
        "provider_failure" => ProjectFailureReason::ProviderFailure,
        "validation_failed" => ProjectFailureReason::ValidationFailed,
        _ => return None,
    })
}
