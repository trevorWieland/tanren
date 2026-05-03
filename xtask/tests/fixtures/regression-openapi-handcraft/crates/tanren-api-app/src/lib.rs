//! Regression fixture for `xtask check-openapi-handcraft`.
//!
//! Mirrors `tanren-api-app/src/` and contains exactly one violation: a
//! hand-rolled OpenAPI document built from a `serde_json::json!` macro
//! literal whose body carries the well-known top-level OpenAPI keys.
//! `check-openapi-handcraft` must reject this fixture; if it stops
//! doing so the guard has been weakened.

pub fn openapi_doc() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.1.0",
        "info": { "title": "tanren", "version": "0.0.0" },
        "paths": {
            "/v1/sign-in": { "post": {} }
        },
        "components": {}
    })
}
