//! End-to-end coverage for `tanren db purge-replay`.
//!
//! Seeds expired and fresh rows directly through the store API, then
//! invokes the CLI subcommand and asserts the JSON stats shape and
//! final ledger state.

use assert_cmd::Command;
use chrono::Utc;
use serde_json::Value;
use tanren_store::{ConsumeActorTokenJtiParams, Store, TokenReplayStore};

fn cli() -> Command {
    Command::cargo_bin("tanren-cli").expect("binary should exist")
}

fn migrated_sqlite_url() -> (String, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let output = cli()
        .args(["--database-url", &url, "db", "migrate"])
        .output()
        .expect("migrate");
    assert!(
        output.status.success(),
        "migrate must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (url, dir)
}

async fn seed(store: &Store, jti: &str, exp_unix: i64) {
    let _ = store
        .consume_actor_token_jti(ConsumeActorTokenJtiParams {
            issuer: "iss".to_owned(),
            audience: "aud".to_owned(),
            jti: jti.to_owned(),
            iat_unix: exp_unix - 10,
            exp_unix,
            consumed_at: Utc::now(),
        })
        .await
        .expect("consume");
}

#[tokio::test]
async fn purge_replay_reports_stats_and_drains_expired_rows() {
    let (db_url, _dir) = migrated_sqlite_url();
    let store = Store::new(&db_url).await.expect("store");

    // Seed 5 expired rows (exp_unix=1) and 1 fresh row (far future).
    for idx in 0..5 {
        seed(&store, &format!("expired-{idx}"), 1).await;
    }
    seed(&store, "fresh", i64::MAX / 2).await;
    drop(store);

    let output = cli()
        .args([
            "--database-url",
            &db_url,
            "db",
            "purge-replay",
            "--batch-limit",
            "2",
            "--retention-secs",
            "1",
        ])
        .output()
        .expect("purge-replay");

    assert!(
        output.status.success(),
        "purge-replay must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let stats: Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(stats["deleted"], 5);
    assert_eq!(stats["remaining_expired"], 0);
    assert!(stats["batches"].as_u64().expect("batches") >= 3);
    assert!(stats.get("lag_seconds").is_some());
}

#[tokio::test]
async fn purge_replay_leaves_unexpired_rows_alone() {
    let (db_url, _dir) = migrated_sqlite_url();
    let store = Store::new(&db_url).await.expect("store");
    seed(&store, "keep-me", i64::MAX / 2).await;
    drop(store);

    let output = cli()
        .args(["--database-url", &db_url, "db", "purge-replay"])
        .output()
        .expect("purge-replay");
    assert!(
        output.status.success(),
        "purge-replay must succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stats: Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(stats["deleted"], 0);
    assert_eq!(stats["remaining_expired"], 0);
}
