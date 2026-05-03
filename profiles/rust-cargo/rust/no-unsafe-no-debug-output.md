---
kind: standard
name: no-unsafe-no-debug-output
category: rust
importance: high
applies_to:
  - "**/*.rs"
applies_to_languages:
  - rust
applies_to_domains:
  - rust
---

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

## Tracing initialization (R-0001)

Every binary in `bin/*/src/main.rs` MUST call
`tanren_observability::init(env_filter)` before doing anything else —
ahead of argument parsing, ahead of config loading, ahead of any spawn
or I/O. The shared init wires `tracing-subscriber` with the JSON
formatter writing to stderr and respects `TANREN_LOG` (or the supplied
filter) for level/filter control.

```rust
// ✓ Good: bin/tanren-cli/src/main.rs
fn main() -> anyhow::Result<()> {
    tanren_observability::init(EnvFilter::from_default_env())?;
    let args = cli::parse();
    cli::run(args)
}
```

```rust
// ✗ Bad: tracing initialized after work has already started
fn main() -> anyhow::Result<()> {
    let args = cli::parse();              // tracing not yet live
    tanren_observability::init(...)?;     // too late
    cli::run(args)
}
```

The CLI and TUI no longer write success/info messages to stdout via
`writeln!`/`println!`. Status information goes through `tracing::info!`
at stderr (JSON formatter). The structured event identifiers needed by
scripts and downstream agents remain on stdout via the existing CLI
output contract — i.e. the contract still defines what bytes go to
stdout, but observational output (progress, success, retries, warnings)
moves to structured tracing on stderr.

Mechanical enforcement: `xtask check-tracing-init` walks the AST of each
`bin/*/src/main.rs` and rejects any file whose `main` does not call
`tanren_observability::init(...)` as its first statement.

**Why:** `unsafe` eliminates a class of memory safety bugs. Structured tracing (vs println) enables filtering, correlation, and machine-parseable logs. Secret wrapping prevents accidental credential exposure in logs, error messages, and debug output. Initializing tracing first means even early-failure paths are observable, and separating structured events (stdout, contract) from observational tracing (stderr, JSON) keeps both audiences served without conflict.
