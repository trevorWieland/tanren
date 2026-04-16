use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseTransaction, DbBackend, EntityTrait, Statement};

use crate::StoreError;
use crate::db_error_codes::{
    extract_db_error_code, is_postgres_unique_violation_code, is_sqlite_unique_violation_code,
};
use crate::entity::actor_token_replay;
use crate::errors::StoreResult;
use crate::params::{ConsumeActorTokenJtiParams, PurgeExpiredActorTokenJtisParams, ReplayGuard};
use crate::store::Store;

#[async_trait]
pub trait TokenReplayStore: Send + Sync {
    /// Consume a token replay key exactly once.
    ///
    /// Returns `Ok(true)` when the key was newly consumed, and
    /// `Ok(false)` when the key already exists (replay).
    async fn consume_actor_token_jti(
        &self,
        params: ConsumeActorTokenJtiParams,
    ) -> StoreResult<bool>;

    /// Best-effort bounded cleanup of expired replay rows.
    async fn purge_expired_actor_token_jtis(
        &self,
        params: PurgeExpiredActorTokenJtisParams,
    ) -> StoreResult<u64>;
}

#[async_trait]
impl TokenReplayStore for Store {
    async fn consume_actor_token_jti(
        &self,
        params: ConsumeActorTokenJtiParams,
    ) -> StoreResult<bool> {
        consume_actor_token_jti_once(self.conn(), params).await
    }

    async fn purge_expired_actor_token_jtis(
        &self,
        params: PurgeExpiredActorTokenJtisParams,
    ) -> StoreResult<u64> {
        if params.limit == 0 {
            return Ok(0);
        }
        let conn = self.conn();
        let backend = conn.get_database_backend();
        // Delete the bounded set of expired rows in a single
        // statement that always uses exactly two binds, regardless
        // of batch size. The previous OR-of-composite-key form
        // emitted three binds per row and could exceed the
        // SQLite bind-variable ceiling on large batches.
        //
        // The SELECT subquery picks the same rows the previous
        // implementation did (oldest expired first, capped at
        // `limit`); the outer DELETE removes exactly that set via
        // their composite primary key, so behavior is equivalent.
        let limit = i64::try_from(params.limit).map_err(|_| StoreError::Conversion {
            context: "token_replay_store::purge_expired_actor_token_jtis",
            reason: "limit exceeds i64::MAX".to_owned(),
        })?;
        let stmt = build_purge_statement(backend, params.expires_before_unix, limit)?;
        let result = conn.execute(stmt).await?;
        Ok(result.rows_affected())
    }
}

fn build_purge_statement(
    backend: DbBackend,
    expires_before_unix: i64,
    limit: i64,
) -> StoreResult<Statement> {
    let sql = match backend {
        DbBackend::Postgres => {
            "DELETE FROM actor_token_replay \
             WHERE (issuer, audience, jti) IN ( \
                 SELECT issuer, audience, jti FROM actor_token_replay \
                 WHERE exp_unix < $1 \
                 ORDER BY exp_unix ASC \
                 LIMIT $2 \
             )"
        }
        DbBackend::Sqlite => {
            "DELETE FROM actor_token_replay \
             WHERE (issuer, audience, jti) IN ( \
                 SELECT issuer, audience, jti FROM actor_token_replay \
                 WHERE exp_unix < ? \
                 ORDER BY exp_unix ASC \
                 LIMIT ? \
             )"
        }
        DbBackend::MySql => {
            return Err(StoreError::Conversion {
                context: "token_replay_store::build_purge_statement",
                reason: "MySQL is not a supported backend".to_owned(),
            });
        }
    };
    Ok(Statement::from_sql_and_values(
        backend,
        sql,
        vec![expires_before_unix.into(), limit.into()],
    ))
}

pub(crate) async fn consume_replay_guard_once(
    txn: &DatabaseTransaction,
    replay_guard: ReplayGuard,
) -> StoreResult<()> {
    let params = replay_guard.into_consume_params(chrono::Utc::now());
    let inserted = consume_actor_token_jti_once(txn, params).await?;
    if inserted {
        Ok(())
    } else {
        Err(StoreError::ReplayRejected)
    }
}

async fn consume_actor_token_jti_once<C>(
    conn: &C,
    params: ConsumeActorTokenJtiParams,
) -> StoreResult<bool>
where
    C: ConnectionTrait,
{
    let row = actor_token_replay::ActiveModel {
        issuer: sea_orm::ActiveValue::Set(params.issuer),
        audience: sea_orm::ActiveValue::Set(params.audience),
        jti: sea_orm::ActiveValue::Set(params.jti),
        iat_unix: sea_orm::ActiveValue::Set(params.iat_unix),
        exp_unix: sea_orm::ActiveValue::Set(params.exp_unix),
        consumed_at: sea_orm::ActiveValue::Set(params.consumed_at),
    };

    match actor_token_replay::Entity::insert(row).exec(conn).await {
        Ok(_) => Ok(true),
        Err(err) => {
            let code = extract_db_error_code(&err);
            if code.as_deref().is_some_and(|c| {
                is_sqlite_unique_violation_code(c) || is_postgres_unique_violation_code(c)
            }) {
                Ok(false)
            } else {
                Err(StoreError::from(err))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::Store;

    #[tokio::test]
    async fn consume_actor_token_is_replay_safe() {
        let store = Store::open_and_migrate("sqlite::memory:")
            .await
            .expect("store");
        let params = ConsumeActorTokenJtiParams {
            issuer: "iss".to_owned(),
            audience: "aud".to_owned(),
            jti: "jti-1".to_owned(),
            iat_unix: 10,
            exp_unix: 20,
            consumed_at: Utc::now(),
        };

        let first = store
            .consume_actor_token_jti(params.clone())
            .await
            .expect("first");
        let second = store.consume_actor_token_jti(params).await.expect("second");
        assert!(first, "first consume should succeed");
        assert!(!second, "second consume should be replay");
    }

    #[tokio::test]
    async fn purge_actor_tokens_is_bounded() {
        let store = Store::open_and_migrate("sqlite::memory:")
            .await
            .expect("store");
        let now = Utc::now();
        for idx in 0..5 {
            let _ = store
                .consume_actor_token_jti(ConsumeActorTokenJtiParams {
                    issuer: "iss".to_owned(),
                    audience: "aud".to_owned(),
                    jti: format!("jti-{idx}"),
                    iat_unix: 10 + idx,
                    exp_unix: 20,
                    consumed_at: now,
                })
                .await
                .expect("insert");
        }
        let deleted = store
            .purge_expired_actor_token_jtis(PurgeExpiredActorTokenJtisParams {
                expires_before_unix: 30,
                limit: 2,
            })
            .await
            .expect("purge");
        assert_eq!(deleted, 2);
    }

    /// The previous OR-of-composite-key purge encoded three binds per
    /// row; a batch of 5000 rows would emit 15 000 binds and exceed
    /// the default `SQLite` bind-variable ceiling on older builds. The
    /// rewritten single-statement DELETE always uses exactly two
    /// binds, so a batch of any reasonable size must succeed.
    #[tokio::test]
    async fn purge_actor_tokens_handles_large_batches_without_bind_explosion() {
        const ROW_COUNT: usize = 5_000;
        let store = Store::open_and_migrate("sqlite::memory:")
            .await
            .expect("store");
        let now = Utc::now();
        for idx in 0..ROW_COUNT {
            let _ = store
                .consume_actor_token_jti(ConsumeActorTokenJtiParams {
                    issuer: "iss".to_owned(),
                    audience: "aud".to_owned(),
                    jti: format!("jti-{idx}"),
                    iat_unix: 10,
                    exp_unix: 20,
                    consumed_at: now,
                })
                .await
                .expect("insert");
        }
        // Cap the batch so the new bind-safe statement is exercised
        // with a non-trivial limit (still 2 binds total).
        let deleted = store
            .purge_expired_actor_token_jtis(PurgeExpiredActorTokenJtisParams {
                expires_before_unix: 30,
                limit: ROW_COUNT as u64,
            })
            .await
            .expect("purge");
        assert_eq!(deleted, ROW_COUNT as u64);
    }
}
