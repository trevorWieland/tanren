use std::time::Duration;

use anyhow::Result;
use reqwest::{Client, Response};
use secrecy::ExposeSecret;
use serde::Deserialize;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, CheckOrganizationPermissionApiRequest,
    CheckOrganizationPermissionResponse, CreateOrganizationApiRequest, CreateOrganizationResponse,
    LIST_ORGANIZATIONS_DEFAULT_LIMIT, ListOrganizationsResponse, SessionEnvelope, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::OrgId;

const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub(super) struct ApiClient {
    base_url: String,
    http: Client,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveSession {
    pub(super) has_token: bool,
}

#[derive(Debug, Clone)]
pub(super) struct SignUpCookieResponse {
    pub(super) account: AccountView,
    pub(super) has_token: bool,
}

#[derive(Debug, Clone)]
pub(super) struct SignInCookieResponse {
    pub(super) account: AccountView,
    pub(super) has_token: bool,
}

#[derive(Debug, Clone)]
pub(super) struct AcceptInvitationCookieResponse {
    pub(super) account: AccountView,
    pub(super) joined_org: OrgId,
    pub(super) has_token: bool,
}

#[derive(Debug, Clone)]
pub(super) enum ApiError {
    Failure { code: String, summary: String },
    Transport(String),
    Decode(String),
}

impl ApiError {
    pub(super) fn message(&self) -> String {
        match self {
            Self::Failure { code, summary } => format!("{code}: {summary}"),
            Self::Transport(summary) | Self::Decode(summary) => {
                format!("internal_error: {summary}")
            }
        }
    }
}

impl ApiClient {
    pub(super) fn new(base_url: String) -> Result<Self> {
        let http = Client::builder()
            .cookie_store(true)
            .timeout(HTTP_TIMEOUT)
            .build()?;
        Ok(Self { base_url, http })
    }

    pub(super) async fn sign_up(
        &self,
        req: SignUpRequest,
    ) -> Result<SignUpCookieResponse, ApiError> {
        let response = self
            .http
            .post(format!("{}/accounts", self.base_url))
            .json(&serde_json::json!({
                "email": req.email.as_str(),
                "password": req.password.expose_secret(),
                "display_name": req.display_name,
            }))
            .send()
            .await
            .map_err(|e| ApiError::Transport(format!("POST /accounts: {e}")))?;
        let payload: AccountCookieResponse = decode_response(response, "POST /accounts").await?;
        Ok(SignUpCookieResponse {
            account: payload.account,
            has_token: session_has_token(&payload.session),
        })
    }

    pub(super) async fn sign_in(
        &self,
        req: SignInRequest,
    ) -> Result<SignInCookieResponse, ApiError> {
        let response = self
            .http
            .post(format!("{}/sessions", self.base_url))
            .json(&serde_json::json!({
                "email": req.email.as_str(),
                "password": req.password.expose_secret(),
            }))
            .send()
            .await
            .map_err(|e| ApiError::Transport(format!("POST /sessions: {e}")))?;
        let payload: AccountCookieResponse = decode_response(response, "POST /sessions").await?;
        Ok(SignInCookieResponse {
            account: payload.account,
            has_token: session_has_token(&payload.session),
        })
    }

    pub(super) async fn accept_invitation(
        &self,
        req: AcceptInvitationRequest,
    ) -> Result<AcceptInvitationCookieResponse, ApiError> {
        let response = self
            .http
            .post(format!(
                "{}/invitations/{}/accept",
                self.base_url,
                req.invitation_token.as_str()
            ))
            .json(&serde_json::json!({
                "email": req.email.as_str(),
                "password": req.password.expose_secret(),
                "display_name": req.display_name,
            }))
            .send()
            .await
            .map_err(|e| ApiError::Transport(format!("POST /invitations/{{token}}/accept: {e}")))?;
        let payload: AcceptInvitationCookieResponseWire =
            decode_response(response, "POST /invitations/{token}/accept").await?;
        Ok(AcceptInvitationCookieResponse {
            account: payload.account,
            joined_org: payload.joined_org,
            has_token: session_has_token(&payload.session),
        })
    }

    pub(super) async fn create_organization(
        &self,
        req: CreateOrganizationApiRequest,
    ) -> Result<CreateOrganizationResponse, ApiError> {
        let response = self
            .http
            .post(format!("{}/organizations", self.base_url))
            .json(&req)
            .send()
            .await
            .map_err(|e| ApiError::Transport(format!("POST /organizations: {e}")))?;
        decode_response(response, "POST /organizations").await
    }

    pub(super) async fn list_organizations(&self) -> Result<ListOrganizationsResponse, ApiError> {
        let url = format!(
            "{}/organizations?limit={}",
            self.base_url, LIST_ORGANIZATIONS_DEFAULT_LIMIT
        );
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| ApiError::Transport(format!("GET /organizations: {e}")))?;
        decode_response(response, "GET /organizations").await
    }

    pub(super) async fn check_organization_permission(
        &self,
        req: CheckOrganizationPermissionApiRequest,
    ) -> Result<CheckOrganizationPermissionResponse, ApiError> {
        let response = self
            .http
            .post(format!("{}/organizations/permissions/check", self.base_url))
            .json(&req)
            .send()
            .await
            .map_err(|e| {
                ApiError::Transport(format!("POST /organizations/permissions/check: {e}"))
            })?;
        decode_response(response, "POST /organizations/permissions/check").await
    }
}

fn session_has_token(session: &SessionEnvelope) -> bool {
    match session {
        SessionEnvelope::Cookie { .. } => true,
        SessionEnvelope::Bearer { token, .. } => !token.expose_secret().is_empty(),
    }
}

async fn decode_response<T>(response: Response, context: &str) -> Result<T, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    let status = response.status();
    if status.is_success() {
        return response
            .json::<T>()
            .await
            .map_err(|e| ApiError::Decode(format!("decode {context} success body: {e}")));
    }

    let body = response
        .json::<FailureBody>()
        .await
        .map_err(|e| ApiError::Decode(format!("decode {context} failure body: {e}")))?;
    Err(ApiError::Failure {
        code: body.code,
        summary: body.summary,
    })
}

#[derive(Debug, Clone, Deserialize)]
struct AccountCookieResponse {
    account: AccountView,
    session: SessionEnvelope,
}

#[derive(Debug, Clone, Deserialize)]
struct AcceptInvitationCookieResponseWire {
    account: AccountView,
    session: SessionEnvelope,
    joined_org: OrgId,
}

#[derive(Debug, Clone, Deserialize)]
struct FailureBody {
    code: String,
    summary: String,
}
