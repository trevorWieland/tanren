use std::sync::Arc;

use anyhow::Result;
use axum::Json;
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use tanren_app_services::{MCP_API_KEY_ENV, McpActorContext, McpAuthConfig, McpAuthFailure};

pub(crate) const API_KEY_ENV: &str = MCP_API_KEY_ENV;

#[derive(Debug, Clone)]
pub(crate) struct AuthConfig {
    inner: McpAuthConfig,
}

impl AuthConfig {
    pub(crate) fn from_env() -> Result<Self> {
        Ok(Self {
            inner: McpAuthConfig::from_env_var(std::env::var(API_KEY_ENV))?,
        })
    }

    pub(crate) fn from_bootstrap_key(api_key: secrecy::SecretString) -> Result<Self> {
        Ok(Self {
            inner: McpAuthConfig::from_bootstrap_key(Some(api_key))?,
        })
    }

    pub(crate) const fn is_configured(&self) -> bool {
        self.inner.is_configured()
    }

    fn authorize(&self, actor: &McpActorContext) -> Result<(), McpAuthFailure> {
        self.inner.authorize(actor)
    }
}

pub(crate) async fn require_api_key(
    axum::extract::State(config): axum::extract::State<Arc<AuthConfig>>,
    request: Request,
    next: Next,
) -> Response {
    let actor = McpActorContext::new(
        request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
        request
            .headers()
            .get("x-api-key")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
    );

    if let Err(reason) = config.authorize(&actor) {
        let status = match reason {
            McpAuthFailure::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            McpAuthFailure::AuthRequired => StatusCode::UNAUTHORIZED,
            McpAuthFailure::PermissionDenied => StatusCode::FORBIDDEN,
        };
        return (status, Json(error_body(reason.code(), reason.summary()))).into_response();
    }

    next.run(request).await
}

fn error_body(code: &str, summary: &str) -> serde_json::Value {
    json!({
        "code": code,
        "summary": summary,
    })
}
