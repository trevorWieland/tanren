//! Smoke check for the R-0015 posture migration and store methods.
//!
//! Connects to an in-memory `SQLite` database (no `DATABASE_URL` needed),
//! applies all pending migrations, then exercises the `PostureStore` trait
//! methods end-to-end: `current_posture`, `set_posture`, and
//! `posture_history`.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example posture_migration_smoke -p tanren-store
//! ```

use anyhow::{Context, Result};
use std::io::Write;
use tanren_domain::Posture;
use tanren_identity_policy::AccountId;
use tanren_store::{PostureStore, Store, StoreError};

#[tokio::main]
async fn main() -> Result<()> {
    let store = Store::connect("sqlite::memory:").await.context("connect")?;
    store.migrate().await.context("migrate")?;

    let actor = AccountId::fresh();

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    let initial = store
        .current_posture()
        .await
        .context("current_posture (initial)")?;
    writeln!(
        handle,
        "current_posture (initial): {:?}",
        initial.as_ref().map(|r| r.posture.to_string())
    )
    .context("write initial")?;
    assert!(initial.is_none(), "expected no posture before first set");

    let first = store
        .set_posture(actor, Posture::LocalOnly, None)
        .await
        .context("set_posture (first)")?;
    writeln!(handle, "set_posture (first): posture={}", first.posture)
        .context("write first set")?;

    let current = store
        .current_posture()
        .await
        .context("current_posture (after first)")?;
    writeln!(
        handle,
        "current_posture (after first): {:?}",
        current.as_ref().map(|r| r.posture.to_string())
    )
    .context("write current after first")?;
    assert_eq!(current.map(|r| r.posture), Some(Posture::LocalOnly));

    let second = store
        .set_posture(actor, Posture::SelfHosted, Some(Posture::LocalOnly))
        .await
        .context("set_posture (second)")?;
    writeln!(handle, "set_posture (second): posture={}", second.posture)
        .context("write second set")?;

    let history = store.posture_history(10).await.context("posture_history")?;
    writeln!(handle, "posture_history count={}", history.len()).context("write history count")?;
    for entry in &history {
        writeln!(
            handle,
            "  - {} -> {} actor={} at={}",
            entry
                .from_posture
                .as_ref()
                .map(ToString::to_string)
                .as_deref()
                .unwrap_or("(none)"),
            entry.to_posture,
            entry.actor.as_uuid(),
            entry.changed_at,
        )
        .context("write history row")?;
    }
    assert_eq!(history.len(), 2, "expected 2 audit rows");
    assert_eq!(history[0].to_posture, Posture::SelfHosted);
    assert_eq!(history[1].to_posture, Posture::LocalOnly);

    let concurrent_result = store
        .set_posture(actor, Posture::Hosted, Some(Posture::LocalOnly))
        .await;
    match concurrent_result {
        Err(StoreError::ConcurrentModification { .. }) => {
            writeln!(handle, "concurrent modification detected as expected")
                .context("write concurrent")?;
        }
        Ok(rec) => {
            anyhow::bail!(
                "expected ConcurrentModification error, but set_posture succeeded with posture={}",
                rec.posture
            );
        }
        Err(other) => {
            anyhow::bail!("unexpected error: {other}");
        }
    }

    writeln!(handle, "posture_migration_smoke: all checks passed").context("write final")?;
    Ok(())
}
