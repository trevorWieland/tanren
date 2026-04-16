//! End-to-end CLI coverage for policy-denied wire shape and the
//! audit-event side effects that must persist regardless of the
//! masked wire response.
//!
//! Addresses audit finding 5: transport-level guarantees for denied
//! create/cancel paths were only validated at the orchestrator layer.
//! These tests drive the policy-denied path through the CLI binary
//! and then inspect the event log directly to confirm the
//! `PolicyDecision` audit event lands.

mod support;

use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::Value;
use support::auth::{
    add_auth_args, assert_stderr_is_single_json, auth_harness, auth_harness_with_org, cli, temp_db,
};
use uuid::Uuid;

/// Count events of the given `event_type` tag recorded against a
/// given dispatch id. Uses a raw parameterized SELECT because the
/// store's entity module is crate-private.
async fn count_events(db_url: &str, dispatch_id: &str, event_type: &str) -> u64 {
    let conn = Database::connect(db_url).await.expect("connect");
    let stmt = Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "SELECT COUNT(*) AS n FROM events \
         WHERE entity_id = ? AND event_type = ?",
        [dispatch_id.to_owned().into(), event_type.to_owned().into()],
    );
    let row = conn.query_one(stmt).await.expect("query");
    let row = row.expect("count row");
    let count: i64 = row.try_get("", "n").expect("n column");
    u64::try_from(count).unwrap_or(0)
}

#[test]
fn create_auto_with_preserve_on_failure_is_denied_with_minimal_wire_payload() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();
    support::auth::lint_anchor(&auth);

    let mut cmd = cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "create",
        "--project",
        "test-project",
        "--phase",
        "do_task",
        "--cli",
        "claude",
        "--branch",
        "main",
        "--spec-folder",
        "spec",
        "--workflow-id",
        "wf-1",
        "--mode",
        "auto",
        "--preserve-on-failure",
    ]);
    add_auth_args(&mut cmd, &auth);

    let output = cmd.output().expect("execute");
    assert!(!output.status.success(), "denied create must exit non-zero");
    let stderr = String::from_utf8(output.stderr).expect("utf8");
    let payload = assert_stderr_is_single_json(&stderr);
    assert_eq!(payload["code"], "policy_denied");
    assert_eq!(payload["message"], "policy denied");

    let details = payload
        .get("details")
        .expect("policy_denied must carry details");
    assert_eq!(details["type"], "policy_denied");
    assert_eq!(
        details["reason_code"],
        "preserve_on_failure_requires_manual_mode"
    );

    // Hard guarantee for Non-Negotiable #13: `policy_denied` wire
    // payloads must never leak resource-identifying metadata.
    let raw = serde_json::to_string(&payload).expect("serialize");
    for banned in [
        "dispatch_id",
        "project",
        "test-project",
        "org_id",
        "user_id",
        "workflow_id",
    ] {
        assert!(
            !raw.contains(banned),
            "policy_denied wire payload must not contain `{banned}`: {raw}"
        );
    }
}

#[tokio::test]
async fn cancel_unauthorized_dispatch_persists_policy_decision_audit_event() {
    let (db_url, _dir) = temp_db();
    let create_auth = auth_harness_with_org(Uuid::now_v7());
    let mut create_cmd = cli();
    create_cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "create",
        "--project",
        "test-project",
        "--phase",
        "do_task",
        "--cli",
        "claude",
        "--branch",
        "main",
        "--spec-folder",
        "spec",
        "--workflow-id",
        "wf-1",
    ]);
    add_auth_args(&mut create_cmd, &create_auth);
    let create_out = create_cmd.output().expect("create");
    assert!(create_out.status.success(), "create must succeed");
    let created: Value = serde_json::from_slice(&create_out.stdout).expect("json");
    let dispatch_id = created["dispatch_id"]
        .as_str()
        .expect("dispatch_id")
        .to_owned();

    // Now attempt cancel under a different org — must be masked as
    // `not_found` on the wire, but a `policy_decision` audit event
    // must land and no `dispatch_cancelled` event may appear.
    let cancel_auth = auth_harness_with_org(Uuid::now_v7());
    let mut cancel_cmd = cli();
    cancel_cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "cancel",
        "--id",
        &dispatch_id,
    ]);
    add_auth_args(&mut cancel_cmd, &cancel_auth);
    let cancel_out = cancel_cmd.output().expect("cancel");
    assert!(
        !cancel_out.status.success(),
        "unauthorized cancel must exit non-zero"
    );
    let stderr = String::from_utf8(cancel_out.stderr).expect("utf8");
    let payload = assert_stderr_is_single_json(&stderr);
    assert_eq!(payload["code"], "not_found");

    let policy_events = count_events(&db_url, &dispatch_id, "policy_decision").await;
    assert!(
        policy_events >= 1,
        "unauthorized cancel must persist a policy_decision audit event"
    );
    let cancelled = count_events(&db_url, &dispatch_id, "dispatch_cancelled").await;
    assert_eq!(
        cancelled, 0,
        "unauthorized cancel must NOT emit a dispatch_cancelled event"
    );
}

#[tokio::test]
async fn cancel_of_missing_dispatch_persists_policy_decision_audit_event() {
    let (db_url, _dir) = temp_db();
    let auth = auth_harness();

    let missing_id = Uuid::now_v7().to_string();
    let mut cmd = cli();
    cmd.args([
        "--database-url",
        &db_url,
        "dispatch",
        "cancel",
        "--id",
        &missing_id,
    ]);
    add_auth_args(&mut cmd, &auth);
    let out = cmd.output().expect("cancel");
    assert!(!out.status.success(), "missing cancel must exit non-zero");
    let stderr = String::from_utf8(out.stderr).expect("utf8");
    let payload = assert_stderr_is_single_json(&stderr);
    assert_eq!(payload["code"], "not_found");

    let policy_events = count_events(&db_url, &missing_id, "policy_decision").await;
    assert_eq!(
        policy_events, 1,
        "missing cancel must persist exactly one policy_decision audit event"
    );
    let cancelled = count_events(&db_url, &missing_id, "dispatch_cancelled").await;
    assert_eq!(
        cancelled, 0,
        "missing cancel must NOT emit a dispatch_cancelled event"
    );
}
