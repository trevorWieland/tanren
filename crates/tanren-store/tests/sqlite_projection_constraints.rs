use sea_orm::{ConnectionTrait, Database};
use tanren_store::Store;

#[tokio::test]
async fn sqlite_projection_guards_reject_invalid_enum_values() {
    let db_path = std::env::temp_dir().join(format!(
        "tanren-store-constraints-{}.db",
        uuid::Uuid::now_v7()
    ));
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let store = Store::new(&url).await.expect("connect");
    store.run_migrations().await.expect("migrate");
    let conn = Database::connect(&url).await.expect("connect raw");

    let dispatch_err = conn
        .execute_unprepared(&format!(
            "INSERT INTO dispatch_projection (dispatch_id, mode, status, outcome, lane, dispatch, actor, graph_revision, user_id, org_id, project, created_at, updated_at) VALUES ('{}', 'manual', 'bogus', NULL, 'impl', '{{}}', '{{}}', 1, '{}', '{}', 'proj', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            uuid::Uuid::now_v7(),
            uuid::Uuid::now_v7(),
            uuid::Uuid::now_v7(),
        ))
        .await
        .expect_err("invalid dispatch status must fail");
    assert!(dispatch_err.to_string().contains("out of enum"));

    let step_err = conn
        .execute_unprepared(&format!(
            "INSERT INTO step_projection (step_id, dispatch_id, step_type, step_sequence, lane, status, ready_state, depends_on, graph_revision, retry_count, created_at, updated_at) VALUES ('{}', '{}', 'bad_kind', 0, 'impl', 'pending', 'ready', '[]', 1, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            uuid::Uuid::now_v7(),
            uuid::Uuid::now_v7(),
        ))
        .await
        .expect_err("invalid step_type must fail");
    assert!(step_err.to_string().contains("out of enum"));

    let _ = std::fs::remove_file(&db_path);
}
