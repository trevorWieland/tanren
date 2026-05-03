---
kind: standard
name: secrets-handling
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - rust
applies_to_domains:
  - architecture
---

# Secrets Handling

Wrap all secrets in `secrecy::Secret<T>`. Never log, serialize, or display raw secret values. Expose only at the point of use.

```rust
// ✓ Good: Secret-wrapped configuration
use secrecy::{ExposeSecret, Secret};

#[derive(Debug)]  // Debug output shows Secret([REDACTED])
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Secret<String>,  // Wrapped
}

pub struct ApiConfig {
    pub endpoint: String,
    pub api_key: Secret<String>,   // Wrapped
    pub webhook_secret: Secret<String>,  // Wrapped
}
```

```rust
// ✓ Good: Expose only at point of use
fn connect(config: &DatabaseConfig) -> Result<Pool> {
    let url = format!(
        "postgres://{}:{}@{}:{}/app",
        config.username,
        config.password.expose_secret(),  // Exposed here only
        config.host,
        config.port,
    );
    Pool::connect(&url).await
}
```

```rust
// ✓ Good: Safe to log structs containing secrets
tracing::info!(?config, "connecting to database");
// Output: config=DatabaseConfig { host: "db.example.com", port: 5432,
//         username: "app", password: Secret([REDACTED]) }
```

```rust
// ✗ Bad: Raw secret in struct
pub struct ApiConfig {
    pub api_key: String,  // Exposed in Debug, logs, error messages
}

// ✗ Bad: Logging raw secret
tracing::info!("connecting with key {}", api_key);

// ✗ Bad: Serializing secret to JSON
#[derive(Serialize)]
pub struct Config {
    pub api_key: String,  // Will appear in JSON output
}
```

**What to wrap:**
- API keys and tokens
- Database passwords and connection strings
- Webhook secrets and signing keys
- OAuth client secrets
- Any credential or authentication material

**Rules:**
- Use `secrecy::Secret<String>` (or `Secret<Vec<u8>>` for binary secrets)
- `Secret<T>` implements `Debug` as `Secret([REDACTED])` — safe to derive `Debug` on containing structs
- Call `.expose_secret()` only at the point where the raw value is needed (HTTP header, connection string)
- Never implement `Display`, `Serialize`, or `ToString` on types that expose raw secrets
- Never pass raw secret values to `tracing` fields or format strings
- Use `secrecy::zeroize` feature if you need memory scrubbing after use

## Mandatory password hashing: Argon2id

The workspace uses `argon2 = "0.5"` (the RustCrypto implementation) for every
password and password-equivalent secret. The single supported entry point is
`Argon2idVerifier::production()`, which defaults to the OWASP 2025 floor:
`m = 19 MiB`, `t = 2`, `p = 1`. Hashes are stored in PHC string format

```
$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>
```

in a single `TEXT` column — there is no separate salt column, because the
salt is embedded in the PHC string and the `argon2` crate parses it back out
on verify.

```rust
// ✓ Good: production verifier with workspace-pinned parameters
let verifier = Argon2idVerifier::production();
let phc = verifier.hash(password.expose_secret())?;
// stored as a single TEXT column on `accounts.password_hash`
```

```rust
// ✓ Good: test-only fast verifier behind a cfg gate
#[cfg(any(test, feature = "test-hooks"))]
let verifier = Argon2idVerifier::fast_for_tests(); // m=8 KiB, t=1, p=1
```

```rust
// ✗ Bad: bare SHA-256 of a password
let digest = sha2::Sha256::new().chain_update(password).finalize();

// ✗ Bad: bcrypt or scrypt — banned in this workspace
let hash = bcrypt::hash(password, 12)?;
```

**Banned alternatives:** bare `sha2::Sha256` for password material, `bcrypt`,
`scrypt`. The workspace `clippy.toml` lists `disallowed_methods` /
`disallowed_types` entries that reject `sha2::Sha256::new` outside the
verifier impl, so accidental misuse fails `just check`.

## Mechanical enforcement: `xtask check-secrets`

An AST walker shipped under `xtask` (run via `just check-secrets`) inspects
every struct field declared anywhere in the workspace. A field is rejected
when its identifier matches the case-insensitive pattern

```
(?i)password|secret|api_key|credential|session_token|bearer|private_key|csrf|auth_token
```

unless its declared type is one of:

- `secrecy::SecretString`
- `secrecy::SecretBox<_>`
- a workspace newtype enumerated in `xtask/secret-newtypes.toml`
  (e.g. `SessionToken`, `InvitationToken`, `ApiKey`)

A field whose name ends in `*token` and whose type is bare `String` is
treated the same way: it must either change type or be added to the
allowlist with a justification comment.

```rust
// ✓ Good: name matches the pattern, type is allow-listed
pub struct AccountRow {
    pub password_hash: SecretString,   // SecretString — OK
    pub session_token: SessionToken,   // newtype listed in secret-newtypes.toml
}

// ✗ Bad: rejected by xtask check-secrets
pub struct AccountRow {
    pub password: String,         // bare String for password-named field
    pub api_key: String,          // bare String for api_key-named field
    pub bearer_token: String,     // *token-named bare String
}
```

**Why:** `Secret<T>` makes accidental credential exposure a compile-time or at minimum a visible code-review concern rather than a silent runtime leak. Argon2id with workspace-pinned parameters keeps every password verification on the same vetted path, and the `xtask` AST walker turns naming-and-typing drift into a hard gate so a careless `pub api_key: String` never reaches main.
