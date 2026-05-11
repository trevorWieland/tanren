//! Loopback guard for test-hook routes.
//!
//! Test-hook routes must only be reachable from loopback addresses.
//! The guard enforces this through two mechanisms:
//!
//! 1. **`ConnectInfo<SocketAddr>`** — available when the server is
//!    started with `into_make_service_with_connect_info::<SocketAddr>()`.
//!    The guard checks that the remote IP is loopback.
//! 2. **`InProcessMarker` extension** — set by `build_app_with_store`
//!    for the in-process BDD harness path. This allows the wire
//!    harness to reach test hooks even when `ConnectInfo` is not yet
//!    provided at the serve call site.
//!
//! The guard does **not** trust `X-Real-IP` or `X-Forwarded-For`
//! headers — those are forgeable by any client.

use std::net::SocketAddr;

use axum::RequestPartsExt;
use axum::extract::ConnectInfo;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Typed marker extension inserted by `build_app_with_store` to signal
/// that the request originates from the in-process BDD harness (which
/// drives the API app over loopback HTTP it spawned itself).
///
/// This is an axum *extension*, not an HTTP header — it cannot be
/// forged by an external client.
#[derive(Clone)]
pub(crate) struct InProcessMarker;

/// Extractor that enforces loopback-or-in-process origin for every
/// test-hook request. Returns `403 Forbidden` if neither condition
/// holds.
pub(crate) struct LoopbackGuard {
    _private: (),
}

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for LoopbackGuard {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        // In-process marker takes priority — it is the explicit bypass
        // for the wire harness that shares the same process.
        if parts.extensions.get::<InProcessMarker>().is_some() {
            return Ok(Self { _private: () });
        }

        // Check ConnectInfo for a loopback remote address.
        let info: Result<ConnectInfo<SocketAddr>, _> =
            parts.extract::<ConnectInfo<SocketAddr>>().await;
        match info {
            Ok(addr) if addr.ip().is_loopback() => Ok(Self { _private: () }),
            Ok(_addr) => Err((
                StatusCode::FORBIDDEN,
                "test-hook route: remote address is not loopback",
            )
                .into_response()),
            Err(_) => Err((
                StatusCode::FORBIDDEN,
                "test-hook route: ConnectInfo unavailable (server must use \
                 into_make_service_with_connect_info)",
            )
                .into_response()),
        }
    }
}
