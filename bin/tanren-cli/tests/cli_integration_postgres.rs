//! Postgres-backed end-to-end CLI integration tests.
//!
//! Mirrors `cli_integration.rs`'s lifecycle coverage but against a real
//! Postgres backend so the full stack (auth → service → orchestrator →
//! store → Postgres SQL) is verified end-to-end.
//!
//! Gated behind the `postgres-integration` cargo feature so default
//! `cargo nextest run -p tanren-cli` runs SQLite-only. Run via
//! `scripts/run_postgres_integration.sh` or the explicit
//! `TANREN_TEST_POSTGRES_URL` environment variable.

#![cfg(feature = "postgres-integration")]

mod support;

use std::time::Duration;

use assert_cmd::Command;
use sea_orm::{ConnectionTrait, Database};
use serde_json::Value;
use support::auth::{add_auth_args, auth_harness_with_org, cli};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres as PostgresImage;
use uuid::Uuid;

const POSTGRES_URL_ENV: &str = "TANREN_TEST_POSTGRES_URL";
const PORT_READINESS_ATTEMPTS: u8 = 30;
const PORT_READINESS_DELAY_MS: u64 = 100;

/// A Postgres fixture — either external (URL) or container-backed.
struct Fixture {
    _container: Option<ContainerAsync<PostgresImage>>,
    url: String,
}

async fn fixture() -> Fixture {
    if let Ok(url) = std::env::var(POSTGRES_URL_ENV) {
        reset_schema(&url).await;
        return Fixture {
            _container: None,
            url,
        };
    }

    let container = PostgresImage::default()
        .start()
        .await
        .unwrap_or_else(|err| {
            panic!(
                "could not start ephemeral Postgres container: {err}. \
             Set {POSTGRES_URL_ENV}=postgres://user:pass@host:port/db to run \
             the CLI integration suite against an external Postgres, or install \
             Docker/Podman/Colima. See scripts/run_postgres_integration.sh."
            );
        });
    let host = resolve_host(&container).await;
    let port = resolve_mapped_port(&container).await;
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

    Fixture {
        _container: Some(container),
        url,
    }
}

async fn resolve_host(container: &ContainerAsync<PostgresImage>) -> String {
    let host = container
        .get_host()
        .await
        .unwrap_or_else(|err| panic!("could not resolve container host: {err}"));
    let raw = host.to_string();
    match raw.as_str() {
        "localhost" | "::" | "::1" => "127.0.0.1".to_owned(),
        _ => raw,
    }
}

async fn resolve_mapped_port(container: &ContainerAsync<PostgresImage>) -> u16 {
    let mut last_err: Option<String> = None;
    for _ in 0..PORT_READINESS_ATTEMPTS {
        match container.get_host_port_ipv4(5432).await {
            Ok(port) => return port,
            Err(err) => {
                last_err = Some(err.to_string());
                tokio::time::sleep(Duration::from_millis(PORT_READINESS_DELAY_MS)).await;
            }
        }
    }
    panic!(
        "cli postgres fixture could not resolve a mapped port after \
         {PORT_READINESS_ATTEMPTS} attempts: {}. Set {POSTGRES_URL_ENV}=postgres://... \
         or re-run on a runtime with host networking.",
        last_err.unwrap_or_else(|| "unknown error".to_owned())
    );
}

async fn reset_schema(url: &str) {
    let conn = Database::connect(url).await.unwrap_or_else(|err| {
        panic!(
            "bootstrap connect to external {POSTGRES_URL_ENV}={url} failed: {err}. \
             Ensure the database is reachable and writable."
        );
    });
    conn.execute_unprepared("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .await
        .unwrap_or_else(|err| panic!("schema reset failed against {url}: {err}"));
}

fn migrate(db_url: &str) {
    let mut cmd: Command = cli();
    cmd.args(["--database-url", db_url, "db", "migrate"]);
    let output = cmd.output().expect("migrate");
    assert!(
        output.status.success(),
        "migrate should succeed against postgres: stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn create_dispatch(db_url: &str, auth: &support::auth::AuthHarness) -> String {
    let mut cmd: Command = cli();
    cmd.args([
        "--database-url",
        db_url,
        "dispatch-mutation",
        "create",
        "--project",
        "postgres-e2e",
        "--phase",
        "do_task",
        "--cli",
        "claude",
        "--branch",
        "main",
        "--spec-folder",
        "spec",
        "--workflow-id",
        "wf-pg-1",
    ]);
    add_auth_args(&mut cmd, auth);

    let output = cmd.output().expect("execute");
    assert!(
        output.status.success(),
        "create should succeed against postgres. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8")
}

fn get_dispatch(db_url: &str, dispatch_id: &str, auth: &support::auth::AuthHarness) -> Value {
    let mut cmd: Command = cli();
    cmd.args([
        "--database-url",
        db_url,
        "dispatch-read",
        "get",
        "--id",
        dispatch_id,
    ]);
    add_auth_args(&mut cmd, auth);
    let output = cmd.output().expect("execute");
    assert!(
        output.status.success(),
        "get should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("json")
}

fn list_dispatches(db_url: &str, auth: &support::auth::AuthHarness) -> Value {
    let mut cmd: Command = cli();
    cmd.args(["--database-url", db_url, "dispatch-read", "list"]);
    add_auth_args(&mut cmd, auth);
    let output = cmd.output().expect("execute");
    assert!(
        output.status.success(),
        "list should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("json")
}

fn cancel_dispatch(db_url: &str, dispatch_id: &str, auth: &support::auth::AuthHarness) -> Value {
    let mut cmd: Command = cli();
    cmd.args([
        "--database-url",
        db_url,
        "dispatch-mutation",
        "cancel",
        "--id",
        dispatch_id,
    ]);
    add_auth_args(&mut cmd, auth);
    let output = cmd.output().expect("execute");
    assert!(
        output.status.success(),
        "cancel should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("json")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_lifecycle_create_get_list_cancel_is_consistent() {
    let fx = fixture().await;
    let db_url = fx.url.clone();
    // Schema creation via the CLI's own `db migrate` path — verifies
    // the migration command works against Postgres end-to-end.
    migrate(&db_url);

    let org_id = Uuid::now_v7();

    let create_auth = auth_harness_with_org(org_id);
    support::auth::lint_anchor(&create_auth);
    let created =
        serde_json::from_str::<Value>(&create_dispatch(&db_url, &create_auth)).expect("json");
    let dispatch_id = created["dispatch_id"]
        .as_str()
        .expect("dispatch_id")
        .to_owned();
    assert_eq!(created["status"], "pending");

    let get_before = get_dispatch(&db_url, &dispatch_id, &auth_harness_with_org(org_id));
    assert_eq!(get_before["dispatch_id"], dispatch_id);
    assert_eq!(get_before["status"], "pending");

    let list_before = list_dispatches(&db_url, &auth_harness_with_org(org_id));
    let pending_entry = list_before["dispatches"]
        .as_array()
        .expect("dispatches array")
        .iter()
        .find(|entry| entry["dispatch_id"] == dispatch_id)
        .expect("created dispatch should appear in list before cancel");
    assert_eq!(pending_entry["status"], "pending");

    let cancel_result = cancel_dispatch(&db_url, &dispatch_id, &auth_harness_with_org(org_id));
    assert_eq!(cancel_result["status"], "cancelled");

    let get_after = get_dispatch(&db_url, &dispatch_id, &auth_harness_with_org(org_id));
    assert_eq!(get_after["dispatch_id"], dispatch_id);
    assert_eq!(get_after["status"], "cancelled");

    let list_after = list_dispatches(&db_url, &auth_harness_with_org(org_id));
    let cancelled_entry = list_after["dispatches"]
        .as_array()
        .expect("dispatches array")
        .iter()
        .find(|entry| entry["dispatch_id"] == dispatch_id)
        .expect("dispatch should still appear in list after cancel");
    assert_eq!(cancelled_entry["status"], "cancelled");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_unauthorized_get_is_hidden_as_not_found() {
    let fx = fixture().await;
    let db_url = fx.url.clone();
    migrate(&db_url);

    let create_auth = auth_harness_with_org(Uuid::now_v7());
    let stdout = create_dispatch(&db_url, &create_auth);
    let created: Value = serde_json::from_str(&stdout).expect("json");
    let dispatch_id = created["dispatch_id"].as_str().expect("dispatch_id");

    let read_auth = auth_harness_with_org(Uuid::now_v7());
    let mut cmd: Command = cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch-read",
        "get",
        "--id",
        dispatch_id,
    ]);
    add_auth_args(&mut cmd, &read_auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "unauthorized read should fail");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let v: Value = serde_json::from_str(&stderr).expect("json");
    assert_eq!(v["code"], "not_found");
}
