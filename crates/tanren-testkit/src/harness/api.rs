//! `@api` harness — spawns `tanren-api-app` on an ephemeral port and
//! drives it via `reqwest::Client` with `cookie_store(true)`.
//!
//! The harness owns the per-scenario `SQLite` database. Reading recent
//! events goes through the harness's own `Store` handle.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderValue;
use reqwest::Client;
use serde_json::Value;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, AccountView, CreateOrgInvitationRequest,
    OrgInvitationView, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{
    AccountId, Identifier, InvitationToken, OrgId, OrganizationPermission,
};
use tanren_store::{AccountStore, CreateInvitation, EventEnvelope, NewInvitation};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessMembershipSeed, HarnessOrgInvitationSeed, HarnessResult, HarnessSession,
};

/// `@api` wire harness.
pub struct ApiHarness {
    base_url: String,
    client: Client,
    store: Arc<Store>,
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

    async fn post_raw(&self, path: &str, body: Value) -> HarnessResult<(Value, bool)> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("POST {path}: {e}")))?;
        let has_cookie = Self::cookie_present(&resp);
        let json: Value = resp
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode: {e}")))?;
        if json.get("code").is_some() {
            return Err(failure_from_body(&json));
        }
        Ok((json, has_cookie))
    }

    async fn get_json(&self, path: &str) -> HarnessResult<Value> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| HarnessError::Transport(format!("GET {path}: {e}")))?;
        let json: Value = resp
            .json()
            .await
            .map_err(|e| HarnessError::Transport(format!("decode: {e}")))?;
        if json.get("code").is_some() {
            return Err(failure_from_body(&json));
        }
        Ok(json)
    }

    fn cookie_present(resp: &reqwest::Response) -> bool {
        resp.headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .any(|v| {
                v.to_str()
                    .ok()
                    .is_some_and(|s| s.starts_with("tanren_session="))
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
        let (json, cookie) = self.post_raw("/accounts", sign_up_body(&req)).await?;
        decode_session(&json, cookie)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let (json, cookie) = self.post_raw("/sessions", sign_in_body(&req)).await?;
        decode_session(&json, cookie)
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let token = req.invitation_token.as_str().to_owned();
        let path = format!("/invitations/{token}/accept");
        let (json, cookie) = self.post_raw(&path, accept_invitation_body(&req)).await?;
        let session = decode_session(&json, cookie)?;
        let joined_org = serde_json::from_value(json["joined_org"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode joined_org: {e}")))?;
        Ok(HarnessAcceptance {
            session,
            joined_org,
        })
    }

    async fn accept_invitations_concurrent(
        &mut self,
        requests: Vec<AcceptInvitationRequest>,
    ) -> Vec<HarnessResult<HarnessAcceptance>> {
        // Fan out via `tokio::spawn` so each acceptance issues its own
        // POST in parallel with its own `reqwest::Client`.
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

    async fn seed_org_invitation(
        &mut self,
        fixture: HarnessOrgInvitationSeed,
    ) -> HarnessResult<()> {
        self.store
            .seed_organization_invitation(CreateInvitation {
                token: fixture.token,
                inviting_org_id: fixture.org_id,
                recipient_identifier: fixture.recipient_identifier,
                granted_permissions: fixture.permissions,
                created_by_account_id: fixture.created_by,
                created_at: chrono::Utc::now(),
                expires_at: fixture.expires_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_org_invitation: {e}")))?;
        Ok(())
    }

    async fn seed_membership(&mut self, fixture: HarnessMembershipSeed) -> HarnessResult<()> {
        self.store
            .insert_membership(
                fixture.account_id,
                fixture.org_id,
                fixture.permissions,
                chrono::Utc::now(),
            )
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_membership: {e}")))?;
        Ok(())
    }

    async fn create_org_invitation(
        &mut self,
        _caller_account_id: AccountId,
        _caller_org_context: Option<OrgId>,
        request: CreateOrgInvitationRequest,
    ) -> HarnessResult<OrgInvitationView> {
        let path = format!("/organizations/{}/invitations", request.org_id.as_uuid());
        let body = serde_json::json!({
            "recipient_identifier": request.recipient_identifier.as_str(),
            "permissions": request.permissions.iter().map(OrganizationPermission::as_str).collect::<Vec<_>>(),
            "expires_at": request.expires_at.to_rfc3339(),
        });
        let (json, _) = self.post_raw(&path, body).await?;
        serde_json::from_value(json["invitation"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode invitation: {e}")))
    }

    async fn list_org_invitations(
        &mut self,
        _caller_account_id: AccountId,
        org_id: OrgId,
    ) -> HarnessResult<Vec<OrgInvitationView>> {
        let path = format!("/organizations/{}/invitations", org_id.as_uuid());
        let json = self.get_json(&path).await?;
        serde_json::from_value(json["invitations"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode invitations: {e}")))
    }

    async fn list_recipient_invitations(
        &mut self,
        identifier: &Identifier,
    ) -> HarnessResult<Vec<OrgInvitationView>> {
        let path = format!("/invitations?recipient_identifier={}", identifier.as_str());
        let json = self.get_json(&path).await?;
        serde_json::from_value(json["invitations"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode invitations: {e}")))
    }

    async fn revoke_invitation(
        &mut self,
        _caller_account_id: AccountId,
        _caller_org_context: Option<OrgId>,
        org_id: OrgId,
        token: InvitationToken,
    ) -> HarnessResult<OrgInvitationView> {
        let path = format!(
            "/organizations/{}/invitations/{}/revoke",
            org_id.as_uuid(),
            token.as_str()
        );
        let (json, _) = self.post_raw(&path, serde_json::json!({})).await?;
        serde_json::from_value(json["invitation"].clone())
            .map_err(|e| HarnessError::Transport(format!("decode invitation: {e}")))
    }

    async fn find_membership_permissions(
        &mut self,
        account_id: AccountId,
        org_id: OrgId,
    ) -> HarnessResult<Vec<OrganizationPermission>> {
        AccountStore::find_organization_permissions(self.store.as_ref(), account_id, org_id)
            .await
            .map_err(|e| HarnessError::Transport(format!("find_membership_permissions: {e}")))
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
        "permission_denied" => AccountFailureReason::PermissionDenied,
        "personal_context" => AccountFailureReason::PersonalContext,
        "invitation_revoked" => AccountFailureReason::InvitationRevoked,
        _ => return None,
    })
}

fn decode_session(json: &Value, has_cookie: bool) -> HarnessResult<HarnessSession> {
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
        has_token: has_cookie,
    })
}
