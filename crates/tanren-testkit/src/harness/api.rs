use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderValue;
use reqwest::Client;
use serde_json::Value;
use tanren_api_app::SignInResponseCookie;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, DeploymentPostureReadModel, DeploymentPostureScope,
    SessionEnvelope, SetDeploymentPostureRequest, SetDeploymentPostureResponse, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::api_support::{
    accept_invitation_body, failure_from_body, has_session_bearer_token, has_session_cookie,
    scenario_db_path, scope_path, sign_in_body, sign_up_body, sqlite_url,
};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessPostureView, HarnessResult, HarnessSession, HarnessSupportedPosture,
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

fn session_envelope_fields(session: &SessionEnvelope) -> (chrono::DateTime<chrono::Utc>, bool) {
    match session {
        SessionEnvelope::Cookie { expires_at, .. } => (*expires_at, false),
        SessionEnvelope::Bearer {
            expires_at, token, ..
        } => (*expires_at, !token.expose_secret().trim().is_empty()),
    }
}

async fn recover_acceptance_from_sign_in(
    client: &Client,
    base_url: &str,
    req: &AcceptInvitationRequest,
) -> Option<HarnessAcceptance> {
    let sign_in_request = SignInRequest {
        email: req.email.clone(),
        password: req.password.clone(),
    };
    let sign_in_url = format!("{base_url}/sessions");
    let sign_in_response = client
        .post(&sign_in_url)
        .json(&sign_in_body(&sign_in_request))
        .send()
        .await
        .ok()?;
    if !sign_in_response.status().is_success() {
        return None;
    }
    let has_cookie = has_session_cookie(sign_in_response.headers());
    let sign_in_json: SignInResponseCookie = sign_in_response.json().await.ok()?;
    let (expires_at, has_bearer_token) = session_envelope_fields(&sign_in_json.session);
    let account = sign_in_json.account;
    let joined_org = account.org?;
    Some(HarnessAcceptance {
        session: HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: has_cookie || has_bearer_token,
        },
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
        let url = format!("{}/accounts", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /accounts: {e}")))?;
        let status = response.status();
        let has_cookie = has_session_cookie(response.headers());
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
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: has_cookie || has_session_bearer_token(&json),
        })
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
        let has_cookie = has_session_cookie(response.headers());
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
        Ok(HarnessSession {
            account_id: account.id,
            account,
            expires_at,
            has_token: has_cookie || has_session_bearer_token(&json),
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
        let has_cookie = has_session_cookie(response.headers());
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            let failure = failure_from_body(&json);
            if let Some(acceptance) =
                recover_acceptance_from_sign_in(&self.client, &self.base_url, &req).await
            {
                return Ok(acceptance);
            }
            return Err(failure);
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
                has_token: has_cookie || has_session_bearer_token(&json),
            },
            joined_org,
        })
    }

    async fn list_supported_postures(&mut self) -> HarnessResult<Vec<HarnessSupportedPosture>> {
        let url = format!("{}/deployment-postures", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("GET /deployment-postures: {e}")))?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let supported = serde_json::from_value(json["supported"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode supported postures: {e}")))?;
        Ok(supported)
    }

    async fn set_deployment_posture(
        &mut self,
        actor: AccountId,
        request: SetDeploymentPostureRequest,
    ) -> HarnessResult<HarnessPostureView> {
        self.set_deployment_posture_raw(actor, request.scope, request.posture.as_wire_value())
            .await
    }

    async fn set_deployment_posture_raw(
        &mut self,
        _actor: AccountId,
        scope: DeploymentPostureScope,
        posture_raw: &str,
    ) -> HarnessResult<HarnessPostureView> {
        let url = format!("{}/deployment-postures", self.base_url);
        let body = serde_json::json!({
            "scope": scope,
            "posture": posture_raw,
        });
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /deployment-postures: {e}")))?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let response: SetDeploymentPostureResponse = serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode posture response: {e}")))?;
        Ok(response.into())
    }

    async fn set_deployment_posture_raw_scope(
        &mut self,
        _actor: AccountId,
        scope_raw: Value,
        posture_raw: &str,
    ) -> HarnessResult<HarnessPostureView> {
        let url = format!("{}/deployment-postures", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "scope": scope_raw, "posture": posture_raw }))
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST /deployment-postures: {e}")))?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let response: SetDeploymentPostureResponse = serde_json::from_value(json)
            .map_err(|e| HarnessError::Transport(format!("decode posture response: {e}")))?;
        Ok(response.into())
    }

    async fn get_deployment_posture(
        &mut self,
        _actor: AccountId,
        scope: DeploymentPostureScope,
    ) -> HarnessResult<Option<HarnessPostureView>> {
        let (scope_kind, scope_id) = scope_path(scope);
        let url = format!(
            "{}/deployment-postures/{scope_kind}/{scope_id}",
            self.base_url
        );
        let response = self.client.get(&url).send().await.map_err(|e| {
            HarnessError::Transport(format!("GET /deployment-postures/{{scope}}: {e}"))
        })?;
        let status = response.status();
        let json: Value = response
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode body: {e}")))?;
        if !status.is_success() {
            return Err(failure_from_body(&json));
        }
        let current: Option<DeploymentPostureReadModel> =
            serde_json::from_value(json["current"].clone())
                .map_err(|e| HarnessError::Transport(format!("decode current posture: {e}")))?;
        Ok(current.map(Into::into))
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
                let has_cookie = has_session_cookie(response.headers());
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
                        has_token: has_cookie || has_session_bearer_token(&json),
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
