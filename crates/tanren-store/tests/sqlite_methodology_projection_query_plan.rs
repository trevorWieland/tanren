//! Query-plan regression tests for methodology projection read paths.

use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use tanren_store::Store;

async fn explain_details(conn: &sea_orm::DatabaseConnection, sql: &str) -> Vec<String> {
    let stmt = Statement::from_string(DbBackend::Sqlite, format!("EXPLAIN QUERY PLAN {sql}"));
    conn.query_all(stmt)
        .await
        .expect("explain query plan")
        .into_iter()
        .map(|row| row.try_get("", "detail").expect("detail"))
        .collect()
}

#[tokio::test]
async fn methodology_projection_queries_use_indexes_without_temp_sort() {
    let db_path = std::env::temp_dir().join(format!(
        "tanren-methodology-projection-plan-{}.db",
        uuid::Uuid::now_v7()
    ));
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let store = Store::new(&db_url).await.expect("connect");
    store.run_migrations().await.expect("migrate");
    let conn = Database::connect(&db_url)
        .await
        .expect("inspect connection");
    conn.execute_unprepared("ANALYZE").await.expect("analyze");

    let entity_details = explain_details(
        &conn,
        "SELECT id FROM events \
         WHERE entity_kind='task' \
           AND entity_id='00000000-0000-0000-0000-000000000011' \
           AND event_type='methodology' \
         ORDER BY timestamp ASC, id ASC \
         LIMIT 1",
    )
    .await;
    let entity_upper = entity_details
        .iter()
        .map(|line| line.to_ascii_uppercase())
        .collect::<Vec<_>>();
    assert!(
        entity_upper.iter().any(|line| {
            line.contains("IX_EVENTS_ENTITY_KIND_ID_TS")
                || line.contains("IX_EVENTS_KIND_ID_TYPE_TS")
        }),
        "expected events composite index usage for entity-scoped recovery: {entity_details:?}"
    );
    assert!(
        entity_upper
            .iter()
            .all(|line| !line.contains("USE TEMP B-TREE")),
        "entity-scoped recovery query must not spill to temp sort: {entity_details:?}"
    );

    let task_spec_details = explain_details(
        &conn,
        "SELECT task_id FROM methodology_task_spec \
         WHERE spec_id='00000000-0000-0000-0000-000000000001'",
    )
    .await;
    let task_spec_upper = task_spec_details
        .iter()
        .map(|line| line.to_ascii_uppercase())
        .collect::<Vec<_>>();
    assert!(
        task_spec_upper
            .iter()
            .any(|line| line.contains("IX_METHODOLOGY_TASK_SPEC_SPEC")),
        "expected task-spec lookup index usage: {task_spec_details:?}"
    );

    let task_list_details = explain_details(
        &conn,
        "SELECT task_id FROM methodology_task_status \
         WHERE spec_id='00000000-0000-0000-0000-000000000001' \
         ORDER BY created_at ASC, task_id ASC",
    )
    .await;
    let task_list_upper = task_list_details
        .iter()
        .map(|line| line.to_ascii_uppercase())
        .collect::<Vec<_>>();
    assert!(
        task_list_upper
            .iter()
            .any(|line| line.contains("IX_METHODOLOGY_TASK_STATUS_SPEC_CREATED")),
        "expected task-list projection index usage: {task_list_details:?}"
    );
    assert!(
        task_list_upper
            .iter()
            .all(|line| !line.contains("USE TEMP B-TREE")),
        "task-list projection query must not spill to temp sort: {task_list_details:?}"
    );

    let _ = std::fs::remove_file(db_path);
}
