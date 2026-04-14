# No Unsafe, No Debug Output

No `unsafe` code. No `println!`/`eprintln!`/`dbg!`. Use `tracing` for all observability. Wrap secrets with `secrecy::Secret<T>`.

```rust
// ✓ Good: Structured tracing with fields
use tracing::{info, warn, instrument};

#[instrument(skip(db), fields(user_id = %user_id))]
pub async fn process_request(user_id: UserId, db: &Database) -> Result<Response> {
    info!("processing request");
    let result = db.query(user_id).await?;
    if result.is_empty() {
        warn!(user_id = %user_id, "no records found");
    }
    Ok(Response::from(result))
}
```

```rust
// ✗ Bad: Debug output
println!("processing user {}", user_id);   // Denied: print_stdout
eprintln!("error: {}", err);               // Denied: print_stderr
dbg!(result);                               // Denied: dbg_macro
```

```rust
// ✓ Good: Secret handling
use secrecy::{ExposeSecret, Secret};

pub struct ApiConfig {
    pub endpoint: String,
    pub api_key: Secret<String>,  // Debug shows Secret([REDACTED])
}

fn connect(config: &ApiConfig) -> Result<Client> {
    // Expose only at point of use
    let header = format!("Bearer {}", config.api_key.expose_secret());
    Client::new(&config.endpoint, header)
}
```

```rust
// ✗ Bad: Raw secret in logs or fields
tracing::info!("connecting with key {}", api_key);  // Leaks secret
```

**Rules:**
- `unsafe_code = "forbid"` — cannot be overridden even with `#[allow]`
- All observability through `tracing` crate with structured fields
- Use `#[instrument]` for automatic span creation on async functions
- `secrecy::Secret<T>` for API keys, tokens, passwords, connection strings
- Never implement `Display` or `Serialize` that exposes raw secret values
- `tracing-subscriber` with `env-filter` and `json` features for production output

**Why:** `unsafe` eliminates a class of memory safety bugs. Structured tracing (vs println) enables filtering, correlation, and machine-parseable logs. Secret wrapping prevents accidental credential exposure in logs, error messages, and debug output.
