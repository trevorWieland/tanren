---
kind: standard
name: thin-binary-crate
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - rust
applies_to_domains:
  - architecture
---

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
- Binary crates should be under 50 lines total — enforced by `just check-thin-binary`
- Binary crates depend on `app-services` or `orchestrator` + concrete infrastructure crates

## Refactor recipe: extract `main()` into a library crate

When a binary creeps over the 50-line ceiling, the fix is to promote its
logic into a per-binary library crate (`crates/tanren-X-app/`) and shrink
`bin/X/src/main.rs` back down to a wiring shell.

```rust
// ✓ Good: library crate owns the runtime
// crates/tanren-X-app/src/lib.rs
pub async fn serve(config: AppConfig) -> anyhow::Result<()> {
    let store = build_store(&config).await?;
    let service = AppService::new(store);
    transport::serve(service, config.port).await
}
```

```rust
// ✓ Good: bin/X/src/main.rs becomes the wiring shell
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tanren_observability::init(cli.log_filter())?;
    tanren_X_app::serve(cli.into_config()).await
}
```

For CLI-shaped binaries the convention is `run()` instead of `serve()`:

```rust
// ✓ Good: CLI variant
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tanren_observability::init(cli.log_filter())?;
    tanren_cli_app::run(cli.into_config()).await
}
```

The 50-line ceiling on `bin/X/src/main.rs` is enforced by
`just check-thin-binary`, which counts non-blank, non-comment lines and
fails the gate when a binary crate exceeds the threshold. Fix the gate
failure by extracting code, never by raising the threshold.

**Why:** Thin binaries keep all testable logic in library crates where it can be unit-tested without spinning up a full server. Multiple binaries (API, CLI, daemon) can share the same library crates with different wiring. The 50-line ceiling forces every non-trivial code path through a library crate the BDD harness can depend on.
