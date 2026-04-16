//! Bounded retention service for the `actor_token_replay` ledger.
//!
//! The replay ledger guards write-path commands against actor-token
//! replay. Rows stay until their `exp_unix` has passed far enough for
//! the operator-configured retention window to elapse. Without a
//! wired cleanup path the table grows unboundedly on busy deployments,
//! which is audit finding 2 of the lane-0.4 review.
//!
//! This module exposes three entry points:
//!
//! - [`ReplayPurgeService::run_once`]: a one-shot bounded purge that
//!   deletes expired rows in fixed-size batches and returns
//!   [`ReplayPurgeStats`] describing the cycle. Safe to invoke from
//!   an explicit CLI maintenance subcommand or from a cron.
//! - [`ReplayPurgeService::run_forever`]: a long-lived driver that
//!   calls `run_once` on an interval until the supplied shutdown
//!   future resolves. Designed for the future `tanren-daemon`
//!   process (lane 0.5+) but runnable from any tokio runtime today.
//! - [`spawn_replay_purge`]: convenience helper that spawns
//!   `run_forever` onto the current runtime and returns its handle.
//!
//! Every purge cycle emits a structured `info!` event on target
//! `tanren_store::replay_purge` carrying `deleted`, `remaining_expired`,
//! `lag_seconds`, `batches`, and `batch_limit` fields — a stable
//! metric surface for any future Prometheus/OTel exporter without
//! adding a new dependency.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::errors::StoreResult;
use crate::params::PurgeExpiredActorTokenJtisParams;
use crate::store::Store;
use crate::token_replay_store::TokenReplayStore;

/// Cap on the number of batches a single `run_once` call will issue.
/// Protects against adversarial clocks and malformed ledger state.
const MAX_LOOP_ITER: u64 = 1_024;

/// Tunables for the replay purge service.
#[derive(Debug, Clone, Copy)]
pub struct ReplayPurgeConfig {
    /// How often `run_forever` calls `run_once`.
    pub interval: Duration,
    /// Minimum age of an expired row before it is eligible for purge.
    /// Measured against `now - exp_unix`.
    pub retention: Duration,
    /// Maximum rows deleted per internal batch.
    pub batch_limit: u64,
    /// Delay before `run_forever` fires its first tick. Lets schema
    /// migrations settle on cold starts.
    pub startup_cooldown: Duration,
}

impl Default for ReplayPurgeConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(300),
            retention: Duration::from_secs(86_400),
            batch_limit: 1_000,
            startup_cooldown: Duration::from_secs(10),
        }
    }
}

/// Statistics for a single `run_once` purge cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ReplayPurgeStats {
    /// Total rows deleted across all internal batches this cycle.
    pub deleted: u64,
    /// Rows still eligible for purge after the cycle ends (0 under
    /// normal operation; nonzero when `MAX_LOOP_ITER` was hit).
    pub remaining_expired: u64,
    /// Oldest unpurged `exp_unix` expressed as seconds-in-the-past
    /// relative to `now()` at cycle start. Negative means "no
    /// remaining expired rows".
    pub lag_seconds: i64,
    /// Number of internal batches issued this cycle.
    pub batches: u64,
}

/// Driver for bounded replay-ledger retention.
#[derive(Debug, Clone)]
pub struct ReplayPurgeService {
    store: Arc<Store>,
    cfg: ReplayPurgeConfig,
}

impl ReplayPurgeService {
    /// Construct a new service with the given store and tunables.
    #[must_use]
    pub fn new(store: Arc<Store>, cfg: ReplayPurgeConfig) -> Self {
        Self { store, cfg }
    }

    /// Run a single bounded purge cycle.
    ///
    /// Deletes rows whose `exp_unix` lies more than
    /// [`ReplayPurgeConfig::retention`] in the past, in batches of
    /// [`ReplayPurgeConfig::batch_limit`]. Emits a structured tracing
    /// event describing the cycle.
    ///
    /// # Errors
    ///
    /// Returns any [`crate::StoreError`] raised by the underlying
    /// store during the purge or the status queries.
    pub async fn run_once(&self) -> StoreResult<ReplayPurgeStats> {
        let now = Utc::now();
        let retention_secs = i64::try_from(self.cfg.retention.as_secs()).unwrap_or(i64::MAX);
        let expires_before_unix = now.timestamp().saturating_sub(retention_secs);

        let mut deleted: u64 = 0;
        let mut batches: u64 = 0;
        for _ in 0..MAX_LOOP_ITER {
            let batch = self
                .store
                .purge_expired_actor_token_jtis(PurgeExpiredActorTokenJtisParams {
                    expires_before_unix,
                    limit: self.cfg.batch_limit,
                })
                .await?;
            batches = batches.saturating_add(1);
            deleted = deleted.saturating_add(batch);
            if batch < self.cfg.batch_limit {
                break;
            }
        }

        let (remaining_expired, lag_seconds) =
            replay_retention_lag(self.store.as_ref(), expires_before_unix, now.timestamp()).await?;

        let stats = ReplayPurgeStats {
            deleted,
            remaining_expired,
            lag_seconds,
            batches,
        };
        emit_tick_metrics(&stats, self.cfg.batch_limit);
        Ok(stats)
    }

    /// Run a forever-loop of purge ticks until `shutdown` resolves.
    ///
    /// Sleeps [`ReplayPurgeConfig::startup_cooldown`] before the first
    /// tick, then alternates between `sleep(interval)` and a
    /// `run_once` call. On error, emits a sanitized `warn!` and keeps
    /// ticking.
    pub async fn run_forever<F>(self, shutdown: F)
    where
        F: std::future::Future<Output = ()> + Send,
    {
        tokio::pin!(shutdown);
        if tokio::time::timeout(self.cfg.startup_cooldown, &mut shutdown)
            .await
            .is_ok()
        {
            return;
        }

        loop {
            if let Err(err) = self.run_once().await {
                // Raw `err` text is emitted via the error's `Display`
                // impl. StoreError never embeds secrets (connection
                // URLs are held opaquely upstream in SeaORM); the
                // calling binary is responsible for sanitizing any
                // further trace sink output.
                warn!(
                    target: "tanren_store::replay_purge",
                    err = %err,
                    "replay_purge_tick_failed"
                );
            }
            if tokio::time::timeout(self.cfg.interval, &mut shutdown)
                .await
                .is_ok()
            {
                return;
            }
        }
    }
}

/// Spawn a long-running purge driver onto the current tokio runtime.
///
/// Returns the join handle so callers can await orderly shutdown.
pub fn spawn_replay_purge<F>(service: ReplayPurgeService, shutdown: F) -> JoinHandle<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    tokio::spawn(service.run_forever(shutdown))
}

async fn replay_retention_lag(
    store: &Store,
    expires_before_unix: i64,
    now_unix: i64,
) -> StoreResult<(u64, i64)> {
    let conn = store.conn();
    let backend = conn.get_database_backend();

    let count_stmt = Statement::from_sql_and_values(
        backend,
        match backend {
            DbBackend::Postgres => {
                "SELECT COUNT(*)::bigint AS n FROM actor_token_replay WHERE exp_unix < $1"
            }
            _ => "SELECT COUNT(*) AS n FROM actor_token_replay WHERE exp_unix < ?",
        },
        vec![expires_before_unix.into()],
    );
    let count_row = conn.query_one(count_stmt).await?;
    let remaining: i64 = count_row
        .as_ref()
        .map(|row| row.try_get::<i64>("", "n"))
        .transpose()?
        .unwrap_or(0);
    let remaining_expired = u64::try_from(remaining).unwrap_or(0);

    if remaining_expired == 0 {
        return Ok((0, -1));
    }

    let min_stmt = Statement::from_sql_and_values(
        backend,
        match backend {
            DbBackend::Postgres => {
                "SELECT MIN(exp_unix)::bigint AS m FROM actor_token_replay \
                 WHERE exp_unix < $1"
            }
            _ => "SELECT MIN(exp_unix) AS m FROM actor_token_replay WHERE exp_unix < ?",
        },
        vec![expires_before_unix.into()],
    );
    let min_row = conn.query_one(min_stmt).await?;
    let min_exp: Option<i64> = min_row
        .as_ref()
        .and_then(|row| row.try_get::<Option<i64>>("", "m").ok().flatten());
    let lag_seconds = min_exp.map_or(-1, |exp| now_unix.saturating_sub(exp));
    Ok((remaining_expired, lag_seconds))
}

fn emit_tick_metrics(stats: &ReplayPurgeStats, batch_limit: u64) {
    info!(
        target: "tanren_store::replay_purge",
        deleted = stats.deleted,
        remaining_expired = stats.remaining_expired,
        lag_seconds = stats.lag_seconds,
        batches = stats.batches,
        batch_limit = batch_limit,
        "replay_purge_tick"
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;
    use tokio::sync::oneshot;

    use super::{ReplayPurgeConfig, ReplayPurgeService};
    use crate::params::ConsumeActorTokenJtiParams;
    use crate::store::Store;
    use crate::token_replay_store::TokenReplayStore;

    async fn seed_expired_rows(store: &Store, count: usize, exp_unix: i64) {
        for idx in 0..count {
            let _ = store
                .consume_actor_token_jti(ConsumeActorTokenJtiParams {
                    issuer: "iss".to_owned(),
                    audience: "aud".to_owned(),
                    jti: format!("jti-expired-{idx}"),
                    iat_unix: exp_unix - 10,
                    exp_unix,
                    consumed_at: Utc::now(),
                })
                .await
                .expect("consume");
        }
    }

    async fn seed_fresh_row(store: &Store, jti: &str, exp_unix: i64) {
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
    async fn run_once_drains_expired_rows() {
        let store = Arc::new(
            Store::open_and_migrate("sqlite::memory:")
                .await
                .expect("store"),
        );
        // Seed 250 expired rows with exp_unix in the distant past and
        // one fresh row with a future exp_unix.
        seed_expired_rows(&store, 250, 1).await;
        seed_fresh_row(&store, "fresh", i64::MAX / 2).await;

        let service = ReplayPurgeService::new(
            store.clone(),
            ReplayPurgeConfig {
                interval: std::time::Duration::from_secs(10),
                retention: std::time::Duration::from_secs(1),
                batch_limit: 100,
                startup_cooldown: std::time::Duration::from_millis(0),
            },
        );
        let stats = service.run_once().await.expect("run_once");
        assert_eq!(stats.deleted, 250);
        assert_eq!(stats.remaining_expired, 0);
        assert!(stats.batches >= 3);
    }

    #[tokio::test]
    async fn run_once_leaves_unexpired_rows_alone() {
        let store = Arc::new(
            Store::open_and_migrate("sqlite::memory:")
                .await
                .expect("store"),
        );
        seed_fresh_row(&store, "keep-me", i64::MAX / 2).await;

        let service = ReplayPurgeService::new(
            store.clone(),
            ReplayPurgeConfig {
                interval: std::time::Duration::from_secs(10),
                retention: std::time::Duration::from_secs(1),
                batch_limit: 10,
                startup_cooldown: std::time::Duration::from_millis(0),
            },
        );
        let stats = service.run_once().await.expect("run_once");
        assert_eq!(stats.deleted, 0);
        assert_eq!(stats.remaining_expired, 0);
    }

    #[tokio::test]
    async fn run_forever_respects_shutdown_during_cooldown() {
        let store = Arc::new(
            Store::open_and_migrate("sqlite::memory:")
                .await
                .expect("store"),
        );
        let service = ReplayPurgeService::new(
            store,
            ReplayPurgeConfig {
                interval: std::time::Duration::from_secs(60),
                retention: std::time::Duration::from_secs(60),
                batch_limit: 10,
                startup_cooldown: std::time::Duration::from_secs(60),
            },
        );
        let (tx, rx) = oneshot::channel::<()>();
        let handle = tokio::spawn(async move {
            service
                .run_forever(async move {
                    let _ = rx.await;
                })
                .await;
        });
        tx.send(()).expect("signal shutdown");
        tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("run_forever must observe shutdown promptly")
            .expect("join");
    }
}
