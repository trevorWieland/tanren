//! Loopback + runtime-secret guard for `/test-hooks/*` routes.
//!
//! Every test-hook request must originate from a loopback address and
//! carry a matching `X-Test-Hook-Secret` header. The secret is generated
//! per-process (or read from `TANREN_TEST_HOOK_SECRET` when the
//! Playwright global-setup supplies one) and never written to a file.

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;

/// Header name for the per-run test-hook secret.
pub(super) const SECRET_HEADER: &str = "x-test-hook-secret";

/// Environment variable used by the Playwright `globalSetup` to
/// inject a shared secret into the API process. When unset the
/// server generates its own and logs it (for manual wiring).
const SECRET_ENV: &str = "TANREN_TEST_HOOK_SECRET";

/// Generate or load the per-run test-hook secret.
pub(super) fn generate_secret() -> String {
    if let Ok(existing) = std::env::var(SECRET_ENV) {
        return existing;
    }
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Axum middleware that rejects non-loopback clients and missing or
/// mismatched secret headers before dispatching test-hook actions.
pub(super) async fn guard_middleware(
    shared_secret: String,
    request: Request,
    next: Next,
) -> Response {
    if !is_loopback(&request) {
        return (
            StatusCode::FORBIDDEN,
            "test-hook routes accept loopback clients only",
        )
            .into_response();
    }
    let provided = request
        .headers()
        .get(SECRET_HEADER)
        .and_then(|v| v.to_str().ok());
    match provided {
        Some(secret) if constant_time_eq(secret, &shared_secret) => {}
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                "missing or mismatched x-test-hook-secret header",
            )
                .into_response();
        }
    }
    next.run(request).await
}

/// Check whether the request originates from a loopback address.
fn is_loopback(request: &Request) -> bool {
    use std::net::IpAddr;
    use std::str::FromStr;

    if let Some(remote) = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    {
        return remote.0.ip().is_loopback();
    }
    // Check X-Real-IP or the first entry in X-Forwarded-For.
    if let Some(real_ip) = request
        .headers()
        .get("x-real-ip")
        .or_else(|| request.headers().get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
    {
        let ip_str = real_ip.split(',').next().unwrap_or("").trim();
        if let Ok(ip) = IpAddr::from_str(ip_str) {
            return ip.is_loopback();
        }
    }
    // In-process (tower::ServiceExt::oneshot) has no remote address — trust.
    true
}

/// Constant-time comparison to avoid timing side-channels on the secret.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}
