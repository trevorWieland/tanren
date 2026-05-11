use std::env;
use std::str::FromStr;
use std::time::Duration;

use reqwest::{Client, Response};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, CheckOrganizationPermissionApiRequest,
    CheckOrganizationPermissionResponse, CreateOrganizationApiRequest, CreateOrganizationResponse,
    LIST_ORGANIZATIONS_DEFAULT_LIMIT, ListOrganizationsResponse, SessionEnvelope, SignInRequest,
    SignUpRequest, organization_permission_options,
};
use tanren_identity_policy::{
    Email, InvitationToken, OrgId, OrganizationName, OrganizationPermission, ValidationError,
};
use uuid::Uuid;

const API_BASE_URL_ENV: &str = "TANREN_API_BASE_URL";
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub(crate) struct ApiClient {
    base_url: String,
    http: Client,
}

#[derive(Debug, Clone)]
pub(crate) struct ActiveSession {
    pub(crate) has_token: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SignUpInput {
    pub(crate) email: String,
    pub(crate) password: SecretString,
    pub(crate) display_name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SignInInput {
    pub(crate) email: String,
    pub(crate) password: SecretString,
}

#[derive(Debug, Clone)]
pub(crate) struct AcceptInvitationInput {
    pub(crate) invitation_token: String,
    pub(crate) email: String,
    pub(crate) password: SecretString,
    pub(crate) display_name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CreateOrganizationInput {
    pub(crate) name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CheckOrganizationPermissionInput {
    pub(crate) org_id: String,
    pub(crate) permission: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SignUpCookieResponse {
    pub(crate) account: AccountView,
    pub(crate) has_token: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SignInCookieResponse {
    pub(crate) account: AccountView,
    pub(crate) has_token: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct AcceptInvitationCookieResponse {
    pub(crate) account: AccountView,
    pub(crate) joined_org: OrgId,
    pub(crate) has_token: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum ApiClientState {
    Available(ApiClient),
    Unavailable(String),
}

impl ApiClientState {
    pub(super) fn from_env() -> Self {
        match env::var(API_BASE_URL_ENV) {
            Ok(base_url) => match ApiClient::new(base_url) {
                Ok(client) => Self::Available(client),
                Err(err) => Self::Unavailable(err.message()),
            },
            Err(_) => {
                Self::Unavailable("TANREN_API_BASE_URL is not set; submit will fail.".to_owned())
            }
        }
    }

    pub(super) fn as_client(&self) -> Option<&ApiClient> {
        match self {
            Self::Available(client) => Some(client),
            Self::Unavailable(_) => None,
        }
    }

    pub(super) fn unavailable_message(&self) -> Option<&str> {
        match self {
            Self::Available(_) => None,
            Self::Unavailable(message) => Some(message.as_str()),
        }
    }
}

impl ApiClient {
    pub(super) fn new(base_url: String) -> Result<Self, ApiError> {
        if base_url.trim().is_empty() {
            return Err(ApiError::Internal(
                "TANREN_API_BASE_URL is not set; submit will fail.".to_owned(),
            ));
        }
        let http = Client::builder()
            .cookie_store(true)
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| ApiError::Internal(format!("build reqwest client: {e}")))?;
        Ok(Self { base_url, http })
    }

    pub(super) async fn sign_up(
        &self,
        input: SignUpInput,
    ) -> Result<SignUpCookieResponse, ApiError> {
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
            .map_err(|e| ApiError::Internal(format!("POST /accounts: {e}")))?;
        let payload: AccountCookieResponse = decode_response(response, "POST /accounts").await?;
        Ok(SignUpCookieResponse {
            account: payload.account,
            has_token: session_has_token(&payload.session),
        })
    }

    pub(super) async fn sign_in(
        &self,
        input: SignInInput,
    ) -> Result<SignInCookieResponse, ApiError> {
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
            .map_err(|e| ApiError::Internal(format!("POST /sessions: {e}")))?;
        let payload: AccountCookieResponse = decode_response(response, "POST /sessions").await?;
        Ok(SignInCookieResponse {
            account: payload.account,
            has_token: session_has_token(&payload.session),
        })
    }

    pub(super) async fn accept_invitation(
        &self,
        input: AcceptInvitationInput,
    ) -> Result<AcceptInvitationCookieResponse, ApiError> {
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
            .map_err(|e| ApiError::Internal(format!("POST /invitations/{{token}}/accept: {e}")))?;
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
        input: CreateOrganizationInput,
    ) -> Result<CreateOrganizationResponse, ApiError> {
        let req = CreateOrganizationApiRequest::new(
            OrganizationName::parse(&input.name).map_err(|e| validation_error(&e))?,
            None,
        );
        let response = self
            .http
            .post(format!("{}/organizations", self.base_url))
            .json(&req)
            .send()
            .await
            .map_err(|e| ApiError::Internal(format!("POST /organizations: {e}")))?;
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
            .map_err(|e| ApiError::Internal(format!("GET /organizations: {e}")))?;
        decode_response(response, "GET /organizations").await
    }

    pub(super) async fn check_organization_permission(
        &self,
        input: CheckOrganizationPermissionInput,
    ) -> Result<CheckOrganizationPermissionResponse, ApiError> {
        let org_id = parse_org_id(&input.org_id)?;
        let permission = parse_permission(&input.permission)?;
        let req = if permission == OrganizationPermission::Configure {
            CheckOrganizationPermissionApiRequest::configure(org_id)
        } else {
            CheckOrganizationPermissionApiRequest::new(org_id, permission)
        };
        let response = self
            .http
            .post(format!("{}/organizations/permissions/check", self.base_url))
            .json(&req)
            .send()
            .await
            .map_err(|e| {
                ApiError::Internal(format!("POST /organizations/permissions/check: {e}"))
            })?;
        decode_response(response, "POST /organizations/permissions/check").await
    }
}

#[derive(Debug, Clone)]
pub(super) enum ApiError {
    Failure { code: String, summary: String },
    Validation(String),
    Internal(String),
}

impl ApiError {
    pub(super) fn message(&self) -> String {
        match self {
            Self::Failure { code, summary } => format!("{code}: {summary}"),
            Self::Validation(summary) => format!("validation_failed: {summary}"),
            Self::Internal(summary) => format!("internal_error: {summary}"),
        }
    }
}

fn session_has_token(session: &SessionEnvelope) -> bool {
    match session {
        SessionEnvelope::Cookie { .. } => true,
        SessionEnvelope::Bearer { token, .. } => !token.expose_secret().is_empty(),
    }
}

fn validation_error(err: &ValidationError) -> ApiError {
    ApiError::Validation(err.to_string())
}

fn parse_org_id(raw: &str) -> Result<OrgId, ApiError> {
    let uuid = Uuid::parse_str(raw.trim()).map_err(|e| ApiError::Validation(e.to_string()))?;
    Ok(OrgId::from(uuid))
}

fn parse_permission(raw: &str) -> Result<OrganizationPermission, ApiError> {
    OrganizationPermission::from_str(raw.trim()).map_err(|_| {
        let expected = organization_permission_options()
            .into_iter()
            .map(OrganizationPermission::as_str)
            .collect::<Vec<_>>()
            .join("|");
        ApiError::Validation(format!("permission must be {expected}"))
    })
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
            .map_err(|e| ApiError::Internal(format!("decode {context} success body: {e}")));
    }

    let body = response
        .json::<FailureBody>()
        .await
        .map_err(|e| ApiError::Internal(format!("decode {context} failure body: {e}")))?;
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
