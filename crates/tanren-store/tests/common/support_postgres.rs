//! Shared Postgres fixture for store integration tests.
//!
//! Handles three environments:
//! 1. Explicit `TANREN_TEST_POSTGRES_URL` — connect + migrate, no container.
//! 2. Container runtime reachable (`docker info` / `podman info`) — spin an
//!    ephemeral Postgres via testcontainers and migrate against it.
//! 3. Neither — panic with a deterministic message pointing at the CI
//!    wrapper script (`scripts/run_postgres_integration.sh`).
//!
//! Port resolution is hardened with a bounded readiness loop to survive
//! container runtimes (Colima, rootless Docker, Podman) that occasionally
//! report `PortNotExposed` before the mapping propagates.
//!
//! Included via `#[path = "support_postgres.rs"] mod support_postgres;`
//! from each Postgres-backed integration test binary so this module
//! only compiles when the `postgres-integration` feature activates those
//! binaries.

use std::time::Duration;

use sea_orm::{ConnectionTrait, Database};
use tanren_store::Store;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres as PostgresImage;

const POSTGRES_URL_ENV: &str = "TANREN_TEST_POSTGRES_URL";
/// Number of readiness probes for resolving the mapped container port.
const PORT_READINESS_ATTEMPTS: u8 = 30;
/// Delay between readiness probes.
const PORT_READINESS_DELAY_MS: u64 = 100;

/// A Postgres fixture — either external (URL) or container-backed.
///
/// The `_container` field keeps the container alive for the lifetime of
/// the fixture when using the container path; dropping it tears the
/// container down.
pub(crate) struct PostgresFixture {
    _container: Option<ContainerAsync<PostgresImage>>,
    pub(crate) url: String,
    pub(crate) store: Store,
}

/// Construct a fresh Postgres fixture with a migrated store.
///
/// When `TANREN_TEST_POSTGRES_URL` is set, resets the `public` schema
/// before migrating. When starting an ephemeral container, the schema
/// is already empty at first use.
pub(crate) async fn postgres_fixture() -> PostgresFixture {
    if let Ok(url) = std::env::var(POSTGRES_URL_ENV) {
        reset_schema(&url).await;
        let store = migrate_fresh(&url).await;
        return PostgresFixture {
            _container: None,
            url,
            store,
        };
    }

    let container = PostgresImage::default()
        .start()
        .await
        .unwrap_or_else(|err| {
            panic!(
                "could not start ephemeral Postgres container: {err}. \
             Set {POSTGRES_URL_ENV}=postgres://user:pass@host:port/db to run \
             the integration suites against an external Postgres, or install \
             Docker/Podman/Colima. See scripts/run_postgres_integration.sh."
            );
        });
    let host = resolve_host(&container).await;
    let port = resolve_mapped_port(&container).await;
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    let store = migrate_fresh(&url).await;

    PostgresFixture {
        _container: Some(container),
        url,
        store,
    }
}

async fn migrate_fresh(url: &str) -> Store {
    let store = Store::new(url).await.unwrap_or_else(|err| {
        panic!("could not connect to postgres at {url}: {err}");
    });
    store
        .run_migrations()
        .await
        .unwrap_or_else(|err| panic!("migration failed against {url}: {err}"));
    store
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

/// Resolve the container host, normalizing IPv6 / `localhost` variants
/// to `127.0.0.1` to avoid IPv6-preference flakes on macOS.
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

/// Resolve the mapped Postgres port with a bounded readiness loop to
/// survive runtimes that briefly return `PortNotExposed` before the
/// port forwarding is fully registered.
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
        "postgres fixture could not resolve a mapped port after {PORT_READINESS_ATTEMPTS} \
         attempts: {}. Set {POSTGRES_URL_ENV}=postgres://... to use an external \
         Postgres, or re-run on a runtime with host networking (docker desktop / \
         colima with port forwarding / podman with `--publish-all`).",
        last_err.unwrap_or_else(|| "unknown error".to_owned())
    );
}
