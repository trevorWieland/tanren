//! MCP runtime configuration and CORS origin parsing.

use std::env;

use axum::http::HeaderValue;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use tokio_util::sync::CancellationToken;

const CORS_ORIGINS_ENV: &str = "TANREN_MCP_CORS_ORIGINS";
const ALLOWED_HOSTS_ENV: &str = "TANREN_MCP_ALLOWED_HOSTS";
const DEFAULT_DEV_ORIGIN: &str = "http://localhost:3000";

#[derive(Debug, Clone)]
pub(crate) struct CorsConfig {
    pub(crate) allow_origins: Vec<HeaderValue>,
}

impl CorsConfig {
    pub(crate) fn from_env() -> Self {
        let allow_origins = parse_cors_origins(env::var(CORS_ORIGINS_ENV).ok().as_deref());
        Self { allow_origins }
    }

    pub(crate) fn test_default() -> Self {
        Self {
            allow_origins: vec![HeaderValue::from_static(DEFAULT_DEV_ORIGIN)],
        }
    }
}

pub(crate) fn streamable_http_config(
    cancellation: CancellationToken,
) -> StreamableHttpServerConfig {
    let base = StreamableHttpServerConfig::default().with_cancellation_token(cancellation);
    let raw = env::var(ALLOWED_HOSTS_ENV).ok().filter(|s| !s.is_empty());
    let Some(value) = raw else {
        return base;
    };
    if value.trim() == "*" {
        tracing::warn!(
            target: "tanren_mcp",
            env_var = ALLOWED_HOSTS_ENV,
            "Host-header validation disabled by `*`; relying on API-key auth as the sole gate."
        );
        return base.disable_allowed_hosts();
    }
    let mut hosts: Vec<String> = vec!["localhost".into(), "127.0.0.1".into(), "::1".into()];
    for host in value.split(',') {
        let trimmed = host.trim();
        if !trimmed.is_empty() {
            hosts.push(trimmed.to_owned());
        }
    }
    tracing::info!(
        target: "tanren_mcp",
        allowed_hosts = ?hosts,
        "Host-header validation extended via {ALLOWED_HOSTS_ENV}"
    );
    base.with_allowed_hosts(hosts)
}

fn parse_cors_origins(raw: Option<&str>) -> Vec<HeaderValue> {
    let trimmed = raw.map_or("", str::trim);
    if trimmed.is_empty() {
        return vec![HeaderValue::from_static(DEFAULT_DEV_ORIGIN)];
    }
    let mut out = Vec::new();
    for token in trimmed.split(',') {
        let origin = token.trim();
        if origin.is_empty() {
            continue;
        }
        if let Ok(value) = HeaderValue::from_str(origin) {
            out.push(value);
        }
    }
    if out.is_empty() {
        return vec![HeaderValue::from_static(DEFAULT_DEV_ORIGIN)];
    }
    out
}
