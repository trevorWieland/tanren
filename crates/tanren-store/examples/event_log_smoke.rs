//! Smoke check for the F-0001 event log primitives.
//!
//! Reads `DATABASE_URL` from the environment, connects to the store, applies
//! pending migrations, appends one event, and reads recent events back.
//! Used during F-0001 sign-off to prove `Store::append_event` and
//! `Store::recent_events` work end-to-end against a live Postgres
//! instance. Replace with typed event commands in the R-* slice that
//! introduces the first concrete event type.
//!
//! Run with:
//!
//! ```sh
//! DATABASE_URL=postgres://postgres:dev@localhost:55432/tanren \
//!   cargo run --example event_log_smoke -p tanren-store
//! ```

use anyhow::{Context, Result};
use chrono::Utc;
use std::env;
use std::io::Write;
use tanren_store::Store;

#[tokio::main]
async fn main() -> Result<()> {
    let url = env::var("DATABASE_URL").context("DATABASE_URL not set")?;
    let store = Store::connect(&url).await.context("connect")?;
    store.migrate().await.context("migrate")?;

    let appended = store
        .append_event(
            serde_json::json!({
                "kind": "f0001-smoke",
                "ok": true,
            }),
            Utc::now(),
        )
        .await
        .context("append_event")?;

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "appended id={} occurred_at={} payload={}",
        appended.id, appended.occurred_at, appended.payload
    )
    .context("write append result")?;

    let recent = store.recent_events(5).await.context("recent_events")?;
    writeln!(handle, "recent_events count={}", recent.len()).context("write count")?;
    for envelope in &recent {
        writeln!(
            handle,
            "  - {} {} {}",
            envelope.id, envelope.occurred_at, envelope.payload
        )
        .context("write recent row")?;
    }
    Ok(())
}
