use std::env;

use tanren_client_integrations::{
    AcceptInvitationCookieResponse, AcceptInvitationInput, CheckOrganizationPermissionInput,
    CreateOrganizationInput, SignInCookieResponse, SignInInput, SignUpCookieResponse, SignUpInput,
    TuiApiClient, TuiClientError,
};
use tanren_contract::{
    CheckOrganizationPermissionResponse, CreateOrganizationResponse, ListOrganizationsResponse,
};

const API_BASE_URL_ENV: &str = "TANREN_API_BASE_URL";

#[derive(Debug, Clone)]
pub(super) struct ApiClient {
    inner: TuiApiClient,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveSession {
    pub(super) has_token: bool,
}

#[derive(Debug, Clone)]
pub(super) enum ApiClientState {
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
        let inner = TuiApiClient::new(base_url).map_err(ApiError::from)?;
        Ok(Self { inner })
    }

    pub(super) async fn sign_up(
        &self,
        input: SignUpInput,
    ) -> Result<SignUpCookieResponse, ApiError> {
        self.inner.sign_up(input).await.map_err(ApiError::from)
    }

    pub(super) async fn sign_in(
        &self,
        input: SignInInput,
    ) -> Result<SignInCookieResponse, ApiError> {
        self.inner.sign_in(input).await.map_err(ApiError::from)
    }

    pub(super) async fn accept_invitation(
        &self,
        input: AcceptInvitationInput,
    ) -> Result<AcceptInvitationCookieResponse, ApiError> {
        self.inner
            .accept_invitation(input)
            .await
            .map_err(ApiError::from)
    }

    pub(super) async fn create_organization(
        &self,
        input: CreateOrganizationInput,
    ) -> Result<CreateOrganizationResponse, ApiError> {
        self.inner
            .create_organization(input)
            .await
            .map_err(ApiError::from)
    }

    pub(super) async fn list_organizations(&self) -> Result<ListOrganizationsResponse, ApiError> {
        self.inner
            .list_organizations()
            .await
            .map_err(ApiError::from)
    }

    pub(super) async fn check_organization_permission(
        &self,
        input: CheckOrganizationPermissionInput,
    ) -> Result<CheckOrganizationPermissionResponse, ApiError> {
        self.inner
            .check_organization_permission(input)
            .await
            .map_err(ApiError::from)
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

impl From<TuiClientError> for ApiError {
    fn from(value: TuiClientError) -> Self {
        match value {
            TuiClientError::Failure { code, summary } => Self::Failure { code, summary },
            TuiClientError::Validation(summary) => Self::Validation(summary),
            TuiClientError::Transport(summary)
            | TuiClientError::Decode(summary)
            | TuiClientError::Configuration(summary) => Self::Internal(summary),
        }
    }
}
