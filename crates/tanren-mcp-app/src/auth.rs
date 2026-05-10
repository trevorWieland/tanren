use std::sync::Arc;

use axum::Json;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use secrecy::SecretString;
use serde_json::json;
use tanren_app_services::{Handlers, SessionAuthenticationRequest, Store};
use tanren_identity_policy::{AccountId, SessionToken};

pub(crate) const API_KEY_ENV: &str = "TANREN_MCP_API_KEY";

#[derive(Debug, Clone)]
pub(crate) struct AuthConfig {
    /// Bootstrap API key. F-0002 sources this from `TANREN_MCP_API_KEY`;
    /// R-0008 will route through the real credential store. Wrapped in
    /// `SecretString` so accidental `Debug` / `Serialize` calls do not
    /// leak the credential.
    pub(crate) bootstrap_key: Option<SecretString>,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthState {
    pub(crate) config: Arc<AuthConfig>,
    pub(crate) store: Arc<Store>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthenticatedPrincipal {
    Bootstrap,
    Account { account_id: AccountId },
}

impl AuthenticatedPrincipal {
    pub(crate) const fn account_id(self) -> Option<AccountId> {
        match self {
            Self::Bootstrap => None,
            Self::Account { account_id } => Some(account_id),
        }
    }

    pub(crate) const fn capability_model(self) -> ActorCapabilityModel {
        match self {
            Self::Bootstrap => ActorCapabilityModel::bootstrap(),
            Self::Account { .. } => ActorCapabilityModel::account(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActorCapabilityModel {
    allow_account_tools: bool,
    allow_setting_tools: bool,
    allow_credential_tools: bool,
}

impl ActorCapabilityModel {
    pub(crate) const fn bootstrap() -> Self {
        Self {
            allow_account_tools: true,
            allow_setting_tools: false,
            allow_credential_tools: false,
        }
    }

    const fn account() -> Self {
        Self {
            allow_account_tools: true,
            allow_setting_tools: true,
            allow_credential_tools: true,
        }
    }

    pub(crate) fn allows_tool(self, tool_name: &str) -> bool {
        match tool_name {
            "account.create" | "account.sign_in" | "account.accept_invitation" => {
                self.allow_account_tools
            }
            "config.user.list" | "config.user.set" | "config.user.remove" => {
                self.allow_setting_tools
            }
            "credential.add" | "credential.update" | "credential.list" | "credential.remove" => {
                self.allow_credential_tools
            }
            _ => false,
        }
    }

    pub(crate) const fn any_tools(self) -> bool {
        self.allow_account_tools || self.allow_setting_tools || self.allow_credential_tools
    }
}

impl AuthConfig {
    pub(crate) fn from_env() -> Self {
        let bootstrap_key = std::env::var(API_KEY_ENV)
            .ok()
            .filter(|s| !s.is_empty())
            .map(SecretString::from);
        Self { bootstrap_key }
    }

    fn extract_credential(headers: &HeaderMap) -> Option<&str> {
        if let Some(value) = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            && let Some(token) = value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
        {
            return Some(token.trim());
        }
        if let Some(value) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
            return Some(value.trim());
        }
        None
    }
}

pub(crate) async fn require_authenticated_principal(
    axum::extract::State(state): axum::extract::State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(presented) = AuthConfig::extract_credential(request.headers()) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(error_body(
                "auth_required",
                "Missing Authorization: Bearer <session-token> or X-API-Key header.",
            )),
        )
            .into_response();
    };

    let principal = match Handlers::new()
        .authenticate_session(
            state.store.as_ref(),
            SessionAuthenticationRequest {
                session_token: SessionToken::from_secret(SecretString::from(presented.to_owned())),
                now: Utc::now(),
            },
        )
        .await
    {
        Ok(Some(session)) => Some(AuthenticatedPrincipal::Account {
            account_id: session.authenticated_account_id,
        }),
        Ok(None) => state
            .config
            .bootstrap_key
            .as_ref()
            .map(secrecy::ExposeSecret::expose_secret)
            .filter(|expected| constant_time_eq(presented.as_bytes(), expected.as_bytes()))
            .map(|_| AuthenticatedPrincipal::Bootstrap),
        Err(err) => {
            tracing::error!(
                target: "tanren_mcp",
                error = %err,
                "failed to authenticate MCP session token"
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body(
                    "internal_error",
                    "Tanren encountered an internal error while authenticating the MCP request.",
                )),
            )
                .into_response();
        }
    };

    let Some(principal) = principal else {
        return (
            StatusCode::FORBIDDEN,
            Json(error_body(
                "permission_denied",
                "Presented credential is not authorized for this MCP service.",
            )),
        )
            .into_response();
    };

    request.extensions_mut().insert(principal);
    next.run(request).await
}

fn error_body(code: &str, summary: &str) -> serde_json::Value {
    json!({
        "code": code,
        "summary": summary,
    })
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
