use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect,
};

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

        let rows = actor_token_replay::Entity::find()
            .filter(actor_token_replay::Column::ExpUnix.lt(params.expires_before_unix))
            .order_by_asc(actor_token_replay::Column::ExpUnix)
            .limit(params.limit)
            .all(self.conn())
            .await?;

        if rows.is_empty() {
            return Ok(0);
        }

        let mut cond = Condition::any();
        for row in rows {
            cond = cond.add(
                Condition::all()
                    .add(actor_token_replay::Column::Issuer.eq(row.issuer))
                    .add(actor_token_replay::Column::Audience.eq(row.audience))
                    .add(actor_token_replay::Column::Jti.eq(row.jti)),
            );
        }

        let result = actor_token_replay::Entity::delete_many()
            .filter(cond)
            .exec(self.conn())
            .await?;
        Ok(result.rows_affected)
    }
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
}
