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

**Why:** `Secret<T>` makes accidental credential exposure a compile-time or at minimum a visible code-review concern rather than a silent runtime leak. The type system prevents secrets from appearing in logs, error messages, serialized output, and debug displays.
