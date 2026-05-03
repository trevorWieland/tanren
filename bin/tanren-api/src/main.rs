//! Tanren HTTP API server.
//!
//! R-0001 wires `tanren-app-services` account-flow handlers behind the
//! cross-interface routes mandated by
//! `docs/architecture/subsystems/interfaces.md`:
//! `POST /accounts` (self-signup), `POST /sessions` (sign-in), and
//! `POST /invitations/:token/accept`. F-0001's `/health` and
//! `/openapi.json` surfaces are preserved.

use std::env;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{Email, InvitationToken};
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:8080";
const BIND_ADDRESS_ENV: &str = "TANREN_API_BIND";
const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Clone)]
struct AppState {
    handlers: Handlers,
    store: Arc<Store>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    version: String,
    contract_version: u32,
}

async fn health() -> Json<HealthResponse> {
    let report = Handlers::new().health(env!("CARGO_PKG_VERSION"));
    Json(HealthResponse {
        status: report.status.to_owned(),
        version: report.version.to_owned(),
        contract_version: report.contract_version.value(),
    })
}

fn openapi_document() -> serde_json::Value {
    let account_failure_schema = json!({
        "type": "object",
        "required": ["code", "summary"],
        "properties": {
            "code": {
                "type": "string",
                "enum": [
                    "duplicate_identifier",
                    "invalid_credential",
                    "invitation_not_found",
                    "invitation_expired",
                    "invitation_already_consumed",
                ]
            },
            "summary": {"type": "string"}
        }
    });
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Tanren API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Tanren control plane for agentic software delivery."
        },
        "paths": {
            "/health": {
                "get": {
                    "summary": "Liveness probe",
                    "responses": {"200": {"description": "Service is live"}}
                }
            },
            "/accounts": {
                "post": {
                    "summary": "Self-signup: create a new personal account",
                    "responses": {
                        "201": {"description": "Account created (returns SignUpResponse)"},
                        "401": {"description": "invalid_credential", "content": {"application/json": {"schema": account_failure_schema}}},
                        "409": {"description": "duplicate_identifier", "content": {"application/json": {"schema": account_failure_schema}}},
                    }
                }
            },
            "/sessions": {
                "post": {
                    "summary": "Sign in: mint a session for an existing account",
                    "responses": {
                        "200": {"description": "Sign-in succeeded (returns SignInResponse)"},
                        "401": {"description": "invalid_credential", "content": {"application/json": {"schema": account_failure_schema}}},
                    }
                }
            },
            "/invitations/{token}/accept": {
                "post": {
                    "summary": "Accept an organization invitation",
                    "responses": {
                        "201": {"description": "Invitation accepted (returns AcceptInvitationResponse)"},
                        "404": {"description": "invitation_not_found", "content": {"application/json": {"schema": account_failure_schema}}},
                        "410": {"description": "invitation_expired or invitation_already_consumed", "content": {"application/json": {"schema": account_failure_schema}}},
                    }
                }
            }
        }
    })
}

async fn serve_openapi() -> Json<serde_json::Value> {
    Json(openapi_document())
}

async fn sign_up_route(
    State(state): State<AppState>,
    Json(request): Json<SignUpRequest>,
) -> Response {
    match state.handlers.sign_up(state.store.as_ref(), request).await {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

async fn sign_in_route(
    State(state): State<AppState>,
    Json(request): Json<SignInRequest>,
) -> Response {
    match state.handlers.sign_in(state.store.as_ref(), request).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

#[derive(Debug, Deserialize)]
struct AcceptInvitationBody {
    email: Email,
    password: String,
    display_name: String,
}

async fn accept_invitation_route(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(body): Json<AcceptInvitationBody>,
) -> Response {
    let invitation_token = match InvitationToken::parse(&token) {
        Ok(t) => t,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"code": "validation_failed", "summary": err.to_string()})),
            )
                .into_response();
        }
    };
    let request = AcceptInvitationRequest {
        invitation_token,
        email: body.email,
        password: SecretString::from(body.password),
        display_name: body.display_name,
    };
    match state
        .handlers
        .accept_invitation(state.store.as_ref(), request)
        .await
    {
        Ok(response) => (StatusCode::CREATED, Json(response)).into_response(),
        Err(err) => map_app_error(err),
    }
}

fn map_app_error(err: AppServiceError) -> Response {
    match err {
        AppServiceError::Account(reason) => failure_body(reason),
        AppServiceError::InvalidInput(message) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"code": "validation_failed", "summary": message})),
        )
            .into_response(),
        AppServiceError::Store(err) => {
            tracing::error!(target: "tanren_api", error = %err, "store error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "code": "internal_error",
                    "summary": "Tanren encountered an internal error.",
                })),
            )
                .into_response()
        }
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "code": "internal_error",
                "summary": "Tanren encountered an internal error.",
            })),
        )
            .into_response(),
    }
}

fn failure_body(reason: AccountFailureReason) -> Response {
    let status =
        StatusCode::from_u16(reason.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(json!({"code": reason.code(), "summary": reason.summary()})),
    )
        .into_response()
}

#[tokio::main]
async fn main() -> Result<()> {
    tanren_observability::init().context("install tracing subscriber")?;

    let bind = env::var(BIND_ADDRESS_ENV).unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_owned());
    let database_url = env::var(DATABASE_URL_ENV).with_context(|| {
        format!("{DATABASE_URL_ENV} must be set so tanren-api can connect to the event store")
    })?;
    let store = Arc::new(
        Store::connect(&database_url)
            .await
            .with_context(|| format!("connect to store at {DATABASE_URL_ENV}"))?,
    );
    let state = AppState {
        handlers: Handlers::new(),
        store,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    let router = Router::new()
        .route("/health", get(health))
        .route("/openapi.json", get(serve_openapi))
        .route("/accounts", post(sign_up_route))
        .route("/sessions", post(sign_in_route))
        .route("/invitations/{token}/accept", post(accept_invitation_route))
        .with_state(state)
        .layer(cors);

    let listener = TcpListener::bind(&bind)
        .await
        .with_context(|| format!("bind {bind}"))?;
    tracing::info!(target: "tanren_api", address = %bind, "tanren-api listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown())
        .await
        .context("axum serve")?;
    Ok(())
}

#[cfg(unix)]
async fn shutdown() {
    use tokio::signal::unix::{SignalKind, signal};
    let sigterm = signal(SignalKind::terminate()).ok();
    if let Some(mut sigterm) = sigterm {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    } else {
        let _ = tokio::signal::ctrl_c().await;
    }
    tracing::info!(target: "tanren_api", "shutdown signal received");
}

#[cfg(not(unix))]
async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!(target: "tanren_api", "shutdown signal received");
}
