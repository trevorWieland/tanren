---
kind: standard
name: cookie-session
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - rust
applies_to_domains:
  - architecture
---

# Cookie Sessions

The API surface authenticates browser clients with a server-stored session
keyed by an opaque cookie. `tower-sessions = "0.14"` is the canonical
session middleware; the cookie carries a session-id pointer only, never
session data, and the row lives in the database so it can be revoked
out-of-band.

## Cookie configuration

```rust
// ✓ Good: hardened session cookie
use tower_sessions::cookie::{time::Duration, SameSite};

let session_layer = SessionManagerLayer::new(store)
    .with_secure(true)
    .with_http_only(true)
    .with_same_site(SameSite::Strict)
    .with_path("/")
    .with_max_age(Duration::days(30)); // Max-Age=2592000
```

```rust
// ✗ Bad: lax cookie defaults
let session_layer = SessionManagerLayer::new(store)
    .with_same_site(SameSite::Lax)   // CSRF surface widens
    .with_secure(false);             // cookie travels over HTTP
```

The flags are non-negotiable: `Secure + HttpOnly + SameSite=Strict +
Path=/ + Max-Age=2592000` (30 days). `SameSite=Strict` is the
user-confirmed default — the email `/invitations/[token]` flow uses a
server-rendered interstitial that submits a same-origin POST so the
Strict cookie fires correctly. There is no need (and no permission) to
relax to `Lax` for cross-site invitation links.

## Session store

Sessions persist in the database, not in the cookie:

```rust
// ✓ Good: DB-backed, revokable session store
use tower_sessions_sqlx_store::PostgresStore;

let store = PostgresStore::new(pool.clone());
store.migrate().await?;
```

The cookie value is just an opaque session id; all account state, expiry,
and revocation status live on the row. Revoking a session is a row
delete + cookie clear, not a token denylist.

## Contract decision: `SessionEnvelope`

The transport — not the user — chooses whether sessions ride a cookie or
a bearer token. The `tanren-contract` crate exposes a single
`SessionEnvelope` enum that surfaces this decision in typed form:

```rust
// ✓ Good: transport-keyed session envelope
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum SessionEnvelope {
    /// API + web responses. The cookie is set by middleware;
    /// the body carries no token.
    Cookie {
        account_id: AccountId,
        expires_at: DateTime<Utc>,
    },
    /// CLI / MCP / TUI responses. No cookie jar, so the token
    /// must travel in the body for the caller to store.
    Bearer {
        account_id: AccountId,
        expires_at: DateTime<Utc>,
        token: SessionToken,
    },
}
```

```rust
// ✗ Bad: leaking the session token in the API response body
#[derive(Serialize)]
struct SignInResponse {
    account_id: AccountId,
    session_token: String, // browser doesn't need this — cookie is set already
}
```

The `Cookie` variant is what the API and web surfaces emit; the `Bearer`
variant is what the CLI, MCP, and TUI binaries emit. The discriminator
is the transport, not a per-user preference, and never overlaps — a
single response is one variant or the other.

## Sign-out

```rust
// ✓ Good: sign-out deletes the row and clears the cookie
async fn revoke(
    session: Session,
    State(store): State<Arc<dyn AccountStore>>,
) -> Result<impl IntoResponse, ApiError> {
    let session_id = session.id().ok_or(ApiError::Unauthenticated)?;
    store.revoke_session(session_id).await?;
    session.flush().await?; // clears server-side state
    Ok((
        // Max-Age=0 instructs the browser to drop the cookie immediately
        clear_session_cookie(),
        StatusCode::NO_CONTENT,
    ))
}
```

`POST /sessions/revoke` is the one-and-only sign-out endpoint. It deletes
the database row and emits a `Set-Cookie` with `Max-Age=0` so the browser
discards its copy. Bearer-mode clients (CLI/MCP/TUI) hit the same endpoint;
the response succeeds whether or not a cookie was present.

**Why:** A DB-backed session with a tiny opaque cookie keeps revocation
authoritative on the server, makes the cookie value useless if exfiltrated
without the matching DB row, and lets `SameSite=Strict` defang CSRF
without contortions in the invitation flow. Encoding the cookie-vs-bearer
choice in the contract type forces every transport to handle session
material correctly at compile time, instead of relying on each handler to
remember which surface it lives on.
