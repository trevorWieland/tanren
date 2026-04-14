# Thin Binary Crate

Binary crates (`main.rs`) are thin orchestration layers. Parse args, initialize tracing, build the dependency graph, delegate to library crates, handle top-level errors. No business logic.

```rust
// ✓ Good: Thin main.rs (~30 lines)
use anyhow::{Context, Result};
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
struct Cli {
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,
    #[arg(long, default_value = "8080")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    let cli = Cli::parse();

    let store = myapp_store::SqliteStore::connect(&cli.database_url)
        .await
        .context("failed to connect to database")?;

    let service = myapp_orchestrator::Service::new(store);

    myapp_api::serve(service, cli.port)
        .await
        .context("server exited with error")
}
```

```rust
// ✗ Bad: Business logic in main.rs
#[tokio::main]
async fn main() -> Result<()> {
    // ... 300 lines of request handling, validation,
    // database queries, error formatting, retries ...
}
```

**What belongs in `main.rs`:**
- CLI argument parsing (clap)
- Tracing/logging initialization
- Configuration loading
- Dependency graph construction (wiring implementations to traits)
- Top-level error handling with `anyhow`
- Signal handling (graceful shutdown)

**What does NOT belong in `main.rs`:**
- Business logic or domain rules
- Data transformation or validation
- Database queries or HTTP request handling
- Retry logic or error recovery
- Any function longer than ~20 lines

**Rules:**
- Binary crates use `anyhow::Result` for the main return type
- All logic lives in library crates, binary crates just wire and run
- Binary crates should be under 100 lines total (often under 50)
- Binary crates depend on `app-services` or `orchestrator` + concrete infrastructure crates

**Why:** Thin binaries keep all testable logic in library crates where it can be unit-tested without spinning up a full server. Multiple binaries (API, CLI, daemon) can share the same library crates with different wiring.
