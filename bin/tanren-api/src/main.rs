//! Tanren HTTP API server.
//!
//! F-0001 ships only the liveness endpoint (`/health`) and a stub
//! `/openapi.json` document. Behavior endpoints arrive with R-* slices,
//! each adding its surface to `tanren-app-services` and a corresponding
//! handler here.

use anyhow::{Context, Result};
use axum::Json;
use axum::Router;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tanren_app_services::Handlers;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

const BIND_ADDRESS: &str = "0.0.0.0:8080";

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
                    "responses": {
                        "200": {
                            "description": "Service is live"
                        }
                    }
                }
            }
        }
    })
}

async fn serve_openapi() -> Json<serde_json::Value> {
    Json(openapi_document())
}

#[tokio::main]
async fn main() -> Result<()> {
    tanren_observability::init().context("install tracing subscriber")?;

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    let router = Router::new()
        .route("/health", get(health))
        .route("/openapi.json", get(serve_openapi))
        .layer(cors);

    let listener = TcpListener::bind(BIND_ADDRESS)
        .await
        .with_context(|| format!("bind {BIND_ADDRESS}"))?;
    tracing::info!(target: "tanren_api", address = BIND_ADDRESS, "tanren-api listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown())
        .await
        .context("axum serve")?;
    Ok(())
}

/// Wait for the first OS signal that indicates the process should shut
/// down gracefully. SIGINT (`Ctrl+C`) is universally supported; SIGTERM
/// is gated on Unix because Windows lacks a direct equivalent.
/// Kubernetes and systemd send SIGTERM during normal rollouts, so the
/// graceful-shutdown path must observe both.
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
