//! Postgres query-plan helpers shared by integration tests.
//!
//! Wraps the `EXPLAIN` and `EXPLAIN ANALYZE` invocations used to
//! inspect the planner's choice for scoped reads, plus the
//! corresponding assertion helpers that distinguish:
//!
//! - exact-index proofs (forced-path) — see
//!   [`assert_scope_index_usage`];
//! - planner-stable performance invariants (natural-planner) — see
//!   [`assert_planner_stable_scope_invariants`].
//!
//! Included via `#[path = "common/postgres_query_plan.rs"]` from
//! the postgres-integration test binaries so this module compiles
//! only under the `postgres-integration` feature.

use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

fn explain_statement(stmt: Statement) -> Statement {
    let explain_sql = format!("EXPLAIN (COSTS FALSE) {}", stmt.sql);
    match stmt.values {
        Some(values) => Statement::from_sql_and_values(DbBackend::Postgres, explain_sql, values),
        None => Statement::from_string(DbBackend::Postgres, explain_sql),
    }
}

/// `EXPLAIN ANALYZE` builder. Surfaces runtime info such as
/// `Sort Method` lines (`quicksort Memory`, `top-N heapsort Memory`,
/// `external merge Disk`, …) so tests can distinguish a bounded-
/// memory sort from a disk-spilling regression without depending on
/// which index the planner picked.
fn explain_analyze_statement(stmt: Statement) -> Statement {
    let explain_sql = format!(
        "EXPLAIN (ANALYZE, COSTS FALSE, BUFFERS FALSE, TIMING FALSE, SUMMARY FALSE) {}",
        stmt.sql
    );
    match stmt.values {
        Some(values) => Statement::from_sql_and_values(DbBackend::Postgres, explain_sql, values),
        None => Statement::from_string(DbBackend::Postgres, explain_sql),
    }
}

/// Run `EXPLAIN` against the live fixture, optionally constraining
/// the planner away from sequential and bitmap paths so callers can
/// prove an index is reachable independent of cost-estimate noise.
pub(crate) async fn explain_plan_lines(
    url: &str,
    stmt: Statement,
    force_index_path: bool,
) -> Vec<String> {
    let conn = Database::connect(url).await.expect("explain connection");
    conn.execute_unprepared("ANALYZE").await.expect("analyze");
    if force_index_path {
        conn.execute_unprepared("SET enable_seqscan = off")
            .await
            .expect("disable seqscan");
        conn.execute_unprepared("SET enable_bitmapscan = off")
            .await
            .expect("disable bitmap scans");
    }
    let rows = conn
        .query_all(explain_statement(stmt))
        .await
        .expect("explain plan");
    plan_rows_to_lines(rows)
}

/// `EXPLAIN ANALYZE` companion to [`explain_plan_lines`]. Runs the
/// query against the fixture so the returned lines include
/// `Sort Method` annotations.
pub(crate) async fn explain_analyze_plan_lines(url: &str, stmt: Statement) -> Vec<String> {
    let conn = Database::connect(url).await.expect("explain connection");
    conn.execute_unprepared("ANALYZE").await.expect("analyze");
    let rows = conn
        .query_all(explain_analyze_statement(stmt))
        .await
        .expect("explain analyze plan");
    plan_rows_to_lines(rows)
}

fn plan_rows_to_lines(rows: Vec<sea_orm::QueryResult>) -> Vec<String> {
    rows.into_iter()
        .map(|row| {
            row.try_get("", "QUERY PLAN")
                .or_else(|_| row.try_get("", "query_plan"))
                .expect("query plan line")
        })
        .collect()
}

/// Strict assertion: the plan must use one of the scoped indexes
/// from m_0006/m_0007/m_0008. Used by the forced-path test where
/// `enable_seqscan = off` constrains the planner.
pub(crate) fn assert_scope_index_usage(lines: &[String]) {
    const ACCEPTED_SCOPE_INDEX_NAMES: [&str; 6] = [
        "IX_DISPATCH_SCOPE_ORG_PROJECT_CREATED_DISPATCH",
        "IX_DISPATCH_SCOPE_TUPLE_CREATED_DISPATCH",
        "IX_DISPATCH_SCOPE_PROJECT",
        "IX_DISPATCH_SCOPE_TEAM",
        "IX_DISPATCH_SCOPE_API_KEY",
        "IX_DISPATCH_ORG_CREATED_DISPATCH",
    ];
    let upper = lines
        .iter()
        .map(|line| line.to_ascii_uppercase())
        .collect::<Vec<_>>();
    assert!(
        upper.iter().any(|line| {
            line.contains("INDEX")
                && ACCEPTED_SCOPE_INDEX_NAMES
                    .iter()
                    .any(|index_name| line.contains(index_name))
        }),
        "expected scoped index in postgres plan: {lines:?}"
    );
}

pub(crate) fn assert_no_seq_scan(lines: &[String]) {
    let upper = lines
        .iter()
        .map(|line| line.to_ascii_uppercase())
        .collect::<Vec<_>>();
    assert!(
        upper.iter().all(|line| !line.contains("SEQ SCAN")),
        "expected no sequential scan for scoped query: {lines:?}"
    );
}

/// Backend-stable invariants for a scoped read under the natural
/// planner. Asserts three properties that survive Postgres planner
/// variation across local Docker, GitHub Actions runners, and
/// different cost-estimate cliffs:
///
/// 1. **No sequential scan** against `dispatch_projection`.
/// 2. **At least one index path** (Index Scan / Index Only Scan /
///    Bitmap Index Scan) appears in the plan.
/// 3. **No disk-spilling Sort Method**. `quicksort Memory` and
///    `top-N heapsort Memory` are acceptable; `external merge Disk`
///    means the index ordering no longer satisfies `ORDER BY` and
///    every matching row must be materialized before emitting a page.
///
/// Crucially this helper does **not** assert which scoped index the
/// planner chose. The forced-path test
/// (`scoped_dispatch_query_plan_uses_scope_indexes_postgres_forced`)
/// already proves the scoped indexes are reachable when planner
/// freedom is constrained. Asserting an exact index family in the
/// natural-planner test was historically brittle on GitHub Actions
/// because the runner's cost estimates differed from local Docker
/// and selected a global index plus filter — a valid but non-scoped
/// plan that nonetheless satisfies all three invariants above.
pub(crate) fn assert_planner_stable_scope_invariants(lines: &[String]) {
    let upper = lines
        .iter()
        .map(|line| line.to_ascii_uppercase())
        .collect::<Vec<_>>();

    assert!(
        upper.iter().all(|line| !line.contains("SEQ SCAN")),
        "scoped query degraded to a sequential scan: {lines:?}"
    );

    let has_index_path = upper.iter().any(|line| {
        line.contains("INDEX SCAN")
            || line.contains("INDEX ONLY SCAN")
            || line.contains("BITMAP INDEX SCAN")
    });
    assert!(
        has_index_path,
        "scoped query did not use any index: {lines:?}"
    );

    let has_disk_sort = upper.iter().any(|line| {
        line.contains("SORT METHOD") && (line.contains("EXTERNAL") || line.contains("DISK"))
    });
    assert!(
        !has_disk_sort,
        "scoped query Sort spilled to disk — index ordering no longer satisfies ORDER BY: {lines:?}"
    );
}
