---
kind: standard
name: openapi-generation
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - rust
applies_to_domains:
  - architecture
---

# OpenAPI Generation

The OpenAPI document for the API surface is **generated from code**, not
hand-written. `utoipa = "5"` is the canonical OpenAPI generator;
`utoipa-axum = "0.2"` provides the `OpenApiRouter` integration that
`tanren-api-app` uses internally. There is exactly one source of truth
for the API contract: the handler signatures and the contract types they
already use.

## Code-first handler annotations

```rust
// ✓ Good: handler carries its own OpenAPI metadata
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;

#[utoipa::path(
    post,
    path = "/v1/sign-in",
    request_body = SignInRequest,
    responses(
        (status = 200, body = SessionEnvelope),
        (status = 401, body = ApiError),
    ),
    tag = "auth",
)]
pub async fn sign_in(
    State(svc): State<Arc<SignInService>>,
    Json(req): Json<SignInRequest>,
) -> Result<Json<SessionEnvelope>, ApiError> { /* ... */ }
```

```rust
// ✓ Good: a single doc struct collects every handler
#[derive(utoipa::OpenApi)]
#[openapi(
    paths(
        sign_in,
        sign_up,
        revoke_session,
        // ...
    ),
    components(schemas(SignInRequest, SessionEnvelope, ApiError)),
)]
pub struct ApiDoc;
```

The `OpenApi` derive on a single doc struct picks up every handler
annotated with `#[utoipa::path(...)]` and emits the document at startup.

## Reuse `JsonSchema` derives

Contract types in `tanren-contract` already derive `JsonSchema` (see the
`id-formats` standard). `utoipa`'s `ToSchema` derive lives alongside it,
so a contract type carries both:

```rust
// ✓ Good: one type, both schemas
#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SignInRequest {
    pub email: Email,
    pub password: SecretString,
}
```

```rust
// ✗ Bad: a parallel "API DTO" copy of a contract type
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SignInRequestDto {
    pub email: String,        // diverges from Email::parse rules
    pub password: String,     // and from SecretString handling
}
```

There is no separate DTO layer. The contract type is the request body
type and the schema source.

## Router wiring

```rust
// ✓ Good: utoipa-axum router integrates into tanren-api-app
use utoipa_axum::router::OpenApiRouter;

pub fn build_router(state: AppState) -> Router {
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(sign_in, sign_up, revoke_session))
        .split_for_parts();

    router
        .merge(serve_openapi_json(api))
        .with_state(state)
}

async fn openapi_json(State(api): State<utoipa::openapi::OpenApi>) -> Json<utoipa::openapi::OpenApi> {
    Json(api)
}
```

The generated document is exposed at `/openapi.json`. Downstream
consumers (the CLI's contract regression tests, the public docs site,
generated client SDKs) read that endpoint or the equivalent
`cargo run --bin tanren-api -- emit-openapi` snapshot — never a
hand-edited file.

## Hand-rolled OpenAPI is forbidden

```rust
// ✗ Bad: hand-rolled OpenAPI as serde_json::json!
async fn openapi_json() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "openapi": "3.1.0",
        "paths": {
            "/v1/sign-in": {
                "post": { /* ... */ }
            }
        }
    }))
}
```

A planned guard, `xtask check-openapi-handcraft`, AST-walks the API
crates and rejects any `serde_json::json!` literal whose top-level keys
include `"openapi"`, `"paths"`, or `"components"`. Until that guard
lands the rule is enforced by review; once landed, raw JSON-literal
OpenAPI definitions fail `just check`.

**Why:** A code-first OpenAPI document derived from the same types the
handlers use makes contract drift impossible — if a handler signature
changes, the schema changes with it. Hand-rolled OpenAPI documents
silently diverge from the running server within a release or two and
become a second source of truth that no one fully trusts. The
`#[utoipa::path]` + `JsonSchema`/`ToSchema` combination keeps the
contract, the schema, and the handler in lockstep at compile time.
