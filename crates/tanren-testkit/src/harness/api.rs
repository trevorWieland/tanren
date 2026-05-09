//! `@api` harness — spawns `tanren-api-app` on an ephemeral port and
//! drives it via `reqwest::Client` with `cookie_store(true)`.

use async_trait::async_trait;
use axum::http::HeaderValue;
use reqwest::Client;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, AccountView, MyPermissionsResponse,
    SignInRequest, SignUpRequest,
};
use tanren_store::{
    AccountStore, EventEnvelope, NewInvitation, NewPermissionConstraint, NewPermissionGrant,
    PermissionGrantScope,
};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessPermissionGrantFixture, HarnessPermissionScope, HarnessPermissionsCapabilityView,
    HarnessPermissionsView, HarnessResult, HarnessSession,
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
        let cookies_set = session_cookie_set(&response);
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        decode_session(&json, cookies_set)
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
        let cookies_set = session_cookie_set(&response);
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        decode_session(&json, cookies_set)
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
        let cookies_set = session_cookie_set(&response);
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        decode_acceptance(&json, cookies_set)
    }

    async fn my_permissions(
        &mut self,
        _session_account_id: tanren_identity_policy::AccountId,
        requested_account_id: Option<tanren_identity_policy::AccountId>,
    ) -> HarnessResult<HarnessPermissionsView> {
        let url = match requested_account_id {
            None => format!("{}/me/permissions", self.base_url),
            Some(target_account_id) => {
                format!("{}/accounts/{target_account_id}/permissions", self.base_url)
            }
        };
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("GET permissions: {e}")))?;
        let status = response.status();
        // Decode once and derive both structured and rendered views from the same bytes.
        let body = response
            .bytes()
            .await
            .map_err(|e| HarnessError::Transport(format!("read permissions body: {e}")))?;
        if !status.is_success() {
            let json: Value = serde_json::from_slice(body.as_ref()).map_err(|e| {
                HarnessError::Transport(format!("decode permissions failure body: {e}"))
            })?;
            return Err(failure_from_body(&json));
        }
        let permissions: MyPermissionsResponse = serde_json::from_slice(body.as_ref())
            .map_err(|e| HarnessError::Transport(format!("decode my_permissions response: {e}")))?;
        Ok(HarnessPermissionsView {
            response: permissions,
            rendered: String::from_utf8_lossy(body.as_ref()).to_string(),
        })
    }

    async fn my_permissions_capability(
        &mut self,
        _session_account_id: tanren_identity_policy::AccountId,
        requested_account_id: Option<tanren_identity_policy::AccountId>,
    ) -> HarnessResult<HarnessPermissionsCapabilityView> {
        let url = match requested_account_id {
            None => format!("{}/me/capabilities", self.base_url),
            Some(target_account_id) => {
                format!(
                    "{}/me/capabilities?account_id={target_account_id}",
                    self.base_url
                )
            }
        };
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("GET permissions capability: {e}")))?;
        let status = response.status();
        let body = response.bytes().await.map_err(|e| {
            HarnessError::Transport(format!("read permissions capability body: {e}"))
        })?;
        if !status.is_success() {
            let json: Value = serde_json::from_slice(body.as_ref()).map_err(|e| {
                HarnessError::Transport(format!("decode permissions capability failure body: {e}"))
            })?;
            return Err(failure_from_body(&json));
        }
        let capabilities = serde_json::from_slice(body.as_ref()).map_err(|e| {
            HarnessError::Transport(format!("decode my_permissions capability response: {e}"))
        })?;
        Ok(HarnessPermissionsCapabilityView {
            response: capabilities,
            rendered: String::from_utf8_lossy(body.as_ref()).to_string(),
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
                let cookies_set = session_cookie_set(&response);
                let json: Value = response
                    .json()
                    .await
                    .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
                if !status.is_success() {
                    return Err(failure_from_body(&json));
                }
                decode_acceptance(&json, cookies_set)
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

    async fn seed_permission_grant(
        &mut self,
        fixture: HarnessPermissionGrantFixture,
    ) -> HarnessResult<()> {
        let scope = match fixture.scope {
            HarnessPermissionScope::Organization(org_id) => {
                PermissionGrantScope::Organization(org_id)
            }
            HarnessPermissionScope::Project(project_id) => {
                PermissionGrantScope::Project(project_id)
            }
        };
        let grant = self
            .store
            .seed_permission_grant(NewPermissionGrant {
                account_id: fixture.account_id,
                scope,
                permission: fixture.permission,
                grant_source: fixture.grant_source,
                created_at: chrono::Utc::now(),
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_permission_grant: {e}")))?;
        if let Some(constraint) = fixture.policy_constraint {
            self.store
                .seed_permission_constraint(NewPermissionConstraint {
                    grant_id: grant.id,
                    reason: constraint.reason,
                    source: constraint.source,
                    created_at: chrono::Utc::now(),
                })
                .await
                .map_err(|e| HarnessError::Transport(format!("seed_permission_constraint: {e}")))?;
        }
        Ok(())
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
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

fn session_cookie_set(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .any(|value| {
            value
                .to_str()
                .ok()
                .is_some_and(|cookie| cookie.starts_with("tanren_session="))
        })
}

fn decode_session(json: &Value, has_token: bool) -> HarnessResult<HarnessSession> {
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
        has_token,
    })
}

fn decode_acceptance(json: &Value, has_token: bool) -> HarnessResult<HarnessAcceptance> {
    let joined_org = serde_json::from_value(json["joined_org"].clone())
        .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
    Ok(HarnessAcceptance {
        session: decode_session(json, has_token)?,
        joined_org,
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
    } else {
        HarnessError::FailureCode { code, summary }
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
