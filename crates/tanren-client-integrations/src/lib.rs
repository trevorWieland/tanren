//! Client Integrations subsystem.
//!
//! Owns *inbound* contracts: webhook receivers, subscriptions, idempotent
//! request semantics, rate limits, and backpressure for external systems
//! that call into Tanren.

use std::str::FromStr;
use std::time::Duration;

use reqwest::{Client, Response};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tanren_contract::{
    AcceptInvitationRequest, AccountView, CheckOrganizationPermissionApiRequest,
    CheckOrganizationPermissionResponse, CreateOrganizationApiRequest, CreateOrganizationResponse,
    LIST_ORGANIZATIONS_DEFAULT_LIMIT, ListOrganizationsResponse, SessionEnvelope, SignInRequest,
    SignUpRequest, organization_permission_options,
};
use tanren_identity_policy::{
    Email, InvitationToken, OrgId, OrganizationName, OrganizationPermission, ValidationError,
};
use thiserror::Error;
use uuid::Uuid;

const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

/// Stable idempotency key supplied by a calling client. Two requests with
/// the same key are treated as the same logical operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Wrap a key string.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the key string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Errors raised by client-integration operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ClientIntegrationError {
    /// The request was rate-limited.
    #[error("rate limited")]
    RateLimited,
}

/// Parsed sign-up fields from the TUI surface.
#[derive(Debug, Clone)]
pub struct SignUpInput {
    pub email: String,
    pub password: SecretString,
    pub display_name: String,
}

/// Parsed sign-in fields from the TUI surface.
#[derive(Debug, Clone)]
pub struct SignInInput {
    pub email: String,
    pub password: SecretString,
}

/// Parsed accept-invitation fields from the TUI surface.
#[derive(Debug, Clone)]
pub struct AcceptInvitationInput {
    pub invitation_token: String,
    pub email: String,
    pub password: SecretString,
    pub display_name: String,
}

/// Parsed create-organization fields from the TUI surface.
#[derive(Debug, Clone)]
pub struct CreateOrganizationInput {
    pub name: String,
}

/// Parsed check-permission fields from the TUI surface.
#[derive(Debug, Clone)]
pub struct CheckOrganizationPermissionInput {
    pub org_id: String,
    pub permission: String,
}

/// TUI response projection for sign-up over cookie-backed HTTP sessions.
#[derive(Debug, Clone)]
pub struct SignUpCookieResponse {
    pub account: AccountView,
    pub has_token: bool,
}

/// TUI response projection for sign-in over cookie-backed HTTP sessions.
#[derive(Debug, Clone)]
pub struct SignInCookieResponse {
    pub account: AccountView,
    pub has_token: bool,
}

/// TUI response projection for invitation acceptance over cookie-backed HTTP sessions.
#[derive(Debug, Clone)]
pub struct AcceptInvitationCookieResponse {
    pub account: AccountView,
    pub joined_org: OrgId,
    pub has_token: bool,
}

/// Typed HTTP + domain-validation client for the TUI app-service boundary.
#[derive(Debug, Clone)]
pub struct TuiApiClient {
    base_url: String,
    http: Client,
}

/// Error model surfaced by [`TuiApiClient`].
#[derive(Debug, Clone, Error)]
pub enum TuiClientError {
    #[error("{code}: {summary}")]
    Failure { code: String, summary: String },
    #[error("validation_failed: {0}")]
    Validation(String),
    #[error("internal_error: {0}")]
    Transport(String),
    #[error("internal_error: {0}")]
    Decode(String),
    #[error("internal_error: {0}")]
    Configuration(String),
}

impl TuiClientError {
    #[must_use]
    pub fn message(&self) -> String {
        self.to_string()
    }
}

impl TuiApiClient {
    /// Build a cookie-jar backed API client for TUI flows.
    pub fn new(base_url: String) -> Result<Self, TuiClientError> {
        if base_url.trim().is_empty() {
            return Err(TuiClientError::Configuration(
                "TANREN_API_BASE_URL is not set; submit will fail.".to_owned(),
            ));
        }
        let http = Client::builder()
            .cookie_store(true)
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| TuiClientError::Transport(format!("build reqwest client: {e}")))?;
        Ok(Self { base_url, http })
    }

    /// Submit sign-up over `POST /accounts`.
    pub async fn sign_up(
        &self,
        input: SignUpInput,
    ) -> Result<SignUpCookieResponse, TuiClientError> {
        let req = SignUpRequest {
            email: Email::parse(&input.email).map_err(|e| validation_error(&e))?,
            password: input.password,
            display_name: input.display_name,
        };
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
            .map_err(|e| TuiClientError::Transport(format!("POST /accounts: {e}")))?;
        let payload: AccountCookieResponse = decode_response(response, "POST /accounts").await?;
        Ok(SignUpCookieResponse {
            account: payload.account,
            has_token: session_has_token(&payload.session),
        })
    }

    /// Submit sign-in over `POST /sessions`.
    pub async fn sign_in(
        &self,
        input: SignInInput,
    ) -> Result<SignInCookieResponse, TuiClientError> {
        let req = SignInRequest {
            email: Email::parse(&input.email).map_err(|e| validation_error(&e))?,
            password: input.password,
        };
        let response = self
            .http
            .post(format!("{}/sessions", self.base_url))
            .json(&serde_json::json!({
                "email": req.email.as_str(),
                "password": req.password.expose_secret(),
            }))
            .send()
            .await
            .map_err(|e| TuiClientError::Transport(format!("POST /sessions: {e}")))?;
        let payload: AccountCookieResponse = decode_response(response, "POST /sessions").await?;
        Ok(SignInCookieResponse {
            account: payload.account,
            has_token: session_has_token(&payload.session),
        })
    }

    /// Submit invitation acceptance over `POST /invitations/{token}/accept`.
    pub async fn accept_invitation(
        &self,
        input: AcceptInvitationInput,
    ) -> Result<AcceptInvitationCookieResponse, TuiClientError> {
        let req = AcceptInvitationRequest {
            invitation_token: InvitationToken::parse(&input.invitation_token)
                .map_err(|e| validation_error(&e))?,
            email: Email::parse(&input.email).map_err(|e| validation_error(&e))?,
            password: input.password,
            display_name: input.display_name,
        };
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
            .map_err(|e| {
                TuiClientError::Transport(format!("POST /invitations/{{token}}/accept: {e}"))
            })?;
        let payload: AcceptInvitationCookieResponseWire =
            decode_response(response, "POST /invitations/{token}/accept").await?;
        Ok(AcceptInvitationCookieResponse {
            account: payload.account,
            joined_org: payload.joined_org,
            has_token: session_has_token(&payload.session),
        })
    }

    /// Submit organization creation over `POST /organizations`.
    pub async fn create_organization(
        &self,
        input: CreateOrganizationInput,
    ) -> Result<CreateOrganizationResponse, TuiClientError> {
        let req = CreateOrganizationApiRequest {
            name: OrganizationName::parse(&input.name).map_err(|e| validation_error(&e))?,
            idempotency_key: None,
        };
        let response = self
            .http
            .post(format!("{}/organizations", self.base_url))
            .json(&req)
            .send()
            .await
            .map_err(|e| TuiClientError::Transport(format!("POST /organizations: {e}")))?;
        decode_response(response, "POST /organizations").await
    }

    /// Submit organization listing over `GET /organizations`.
    pub async fn list_organizations(&self) -> Result<ListOrganizationsResponse, TuiClientError> {
        let url = format!(
            "{}/organizations?limit={}",
            self.base_url, LIST_ORGANIZATIONS_DEFAULT_LIMIT
        );
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| TuiClientError::Transport(format!("GET /organizations: {e}")))?;
        decode_response(response, "GET /organizations").await
    }

    /// Submit permission check over `POST /organizations/permissions/check`.
    pub async fn check_organization_permission(
        &self,
        input: CheckOrganizationPermissionInput,
    ) -> Result<CheckOrganizationPermissionResponse, TuiClientError> {
        let req = CheckOrganizationPermissionApiRequest {
            org_id: parse_org_id(&input.org_id)?,
            permission: parse_permission(&input.permission)?,
        };
        let response = self
            .http
            .post(format!("{}/organizations/permissions/check", self.base_url))
            .json(&req)
            .send()
            .await
            .map_err(|e| {
                TuiClientError::Transport(format!("POST /organizations/permissions/check: {e}"))
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

fn validation_error(err: &ValidationError) -> TuiClientError {
    TuiClientError::Validation(err.to_string())
}

fn parse_org_id(raw: &str) -> Result<OrgId, TuiClientError> {
    let uuid =
        Uuid::parse_str(raw.trim()).map_err(|e| TuiClientError::Validation(e.to_string()))?;
    Ok(OrgId::from(uuid))
}

fn parse_permission(raw: &str) -> Result<OrganizationPermission, TuiClientError> {
    OrganizationPermission::from_str(raw.trim()).map_err(|_| {
        let expected = organization_permission_options()
            .into_iter()
            .map(OrganizationPermission::as_str)
            .collect::<Vec<_>>()
            .join("|");
        TuiClientError::Validation(format!("permission must be {expected}"))
    })
}

async fn decode_response<T>(response: Response, context: &str) -> Result<T, TuiClientError>
where
    T: serde::de::DeserializeOwned,
{
    let status = response.status();
    if status.is_success() {
        return response
            .json::<T>()
            .await
            .map_err(|e| TuiClientError::Decode(format!("decode {context} success body: {e}")));
    }

    let body = response
        .json::<FailureBody>()
        .await
        .map_err(|e| TuiClientError::Decode(format!("decode {context} failure body: {e}")))?;
    Err(TuiClientError::Failure {
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
