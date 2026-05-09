//! `SeaORM`-backed implementation of atomic active-account switching.
//!
//! The flow persists the latest active-account scope state and appends
//! the canonical `active_account_switched` event inside one DB
//! transaction. Any failure rolls both writes back.

use chrono::{DateTime, Utc};
use sea_orm::sea_query;
use sea_orm::sea_query::{Expr, Iden, Query};
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr, Set,
    TransactionTrait,
};
use tanren_identity_policy::AccountId;

use crate::StoreError;
use crate::entity;
use crate::traits::{SwitchActiveAccountAtomicError, SwitchActiveAccountAtomicRequest};

pub(crate) async fn run(
    conn: &DatabaseConnection,
    request: SwitchActiveAccountAtomicRequest,
) -> Result<(), SwitchActiveAccountAtomicError> {
    conn.transaction::<_, (), SwitchActiveAccountAtomicError>(|txn| {
        Box::pin(async move { run_in_txn(txn, request).await })
    })
    .await
    .map_err(map_transaction_error)
}

async fn run_in_txn(
    txn: &DatabaseTransaction,
    request: SwitchActiveAccountAtomicRequest,
) -> Result<(), SwitchActiveAccountAtomicError> {
    let SwitchActiveAccountAtomicRequest {
        from_account_id,
        to_account_id,
        signed_in_account_ids,
        now,
        switched_event_payload,
    } = request;

    if signed_in_account_ids.is_empty() {
        return Err(SwitchActiveAccountAtomicError::EmptySignedInSet);
    }

    if !signed_in_account_ids.contains(&from_account_id) {
        return Err(SwitchActiveAccountAtomicError::ActiveAccountNotSignedIn);
    }

    if !signed_in_account_ids.contains(&to_account_id) {
        return Err(SwitchActiveAccountAtomicError::TargetAccountNotSignedIn);
    }

    persist_active_scope_row_in_txn(txn, &signed_in_account_ids, to_account_id, now).await?;

    let event_model = entity::events::ActiveModel {
        id: Set(uuid::Uuid::now_v7()),
        occurred_at: Set(now),
        payload: Set(switched_event_payload),
    };
    event_model.insert(txn).await.map_err(StoreError::from)?;

    Ok(())
}

async fn persist_active_scope_row_in_txn(
    txn: &DatabaseTransaction,
    signed_in_account_ids: &[AccountId],
    active_account_id: AccountId,
    now: DateTime<Utc>,
) -> Result<(), SwitchActiveAccountAtomicError> {
    let scope_hash = scope_hash_for_signed_in_ids(signed_in_account_ids);
    let signed_in_snapshot = signed_in_snapshot_for_scope(signed_in_account_ids);

    let update = Query::update()
        .table(ActiveAccountScopes::Table)
        .values([
            (
                ActiveAccountScopes::ActiveAccountId,
                active_account_id.as_uuid().into(),
            ),
            (
                ActiveAccountScopes::SignedInAccountIds,
                signed_in_snapshot.clone().into(),
            ),
            (ActiveAccountScopes::UpdatedAt, now.into()),
        ])
        .and_where(Expr::col(ActiveAccountScopes::ScopeHash).eq(scope_hash.clone()))
        .to_owned();

    let backend = txn.get_database_backend();
    let updated = txn
        .execute(backend.build(&update))
        .await
        .map_err(StoreError::from)?;
    if updated.rows_affected() > 0 {
        return Ok(());
    }

    let insert = Query::insert()
        .into_table(ActiveAccountScopes::Table)
        .columns([
            ActiveAccountScopes::ScopeHash,
            ActiveAccountScopes::ActiveAccountId,
            ActiveAccountScopes::SignedInAccountIds,
            ActiveAccountScopes::UpdatedAt,
        ])
        .values_panic([
            scope_hash.clone().into(),
            active_account_id.as_uuid().into(),
            signed_in_snapshot.into(),
            now.into(),
        ])
        .to_owned();

    match txn.execute(backend.build(&insert)).await {
        Ok(_) => Ok(()),
        // Race-safe fallback: if another transaction inserted the row
        // between our update and insert, rerun the update in this
        // transaction so the latest switch still wins.
        Err(insert_err) if is_unique_violation(&insert_err) => {
            txn.execute(backend.build(&update))
                .await
                .map_err(StoreError::from)?;
            Ok(())
        }
        Err(insert_err) => Err(SwitchActiveAccountAtomicError::Store(StoreError::from(
            insert_err,
        ))),
    }
}

fn scope_hash_for_signed_in_ids(account_ids: &[AccountId]) -> String {
    let mut tokens = account_ids
        .iter()
        .map(|account_id| account_id.as_uuid().as_hyphenated().to_string())
        .collect::<Vec<_>>();
    tokens.sort_unstable();
    tokens.dedup();
    tokens.join(",")
}

fn signed_in_snapshot_for_scope(account_ids: &[AccountId]) -> String {
    let mut tokens = account_ids
        .iter()
        .map(|account_id| account_id.as_uuid().as_hyphenated().to_string())
        .collect::<Vec<_>>();
    tokens.sort_unstable();
    tokens.dedup();
    tokens.join(",")
}

fn is_unique_violation(err: &DbErr) -> bool {
    let message = err.to_string().to_lowercase();
    message.contains("unique") || message.contains("duplicate")
}

fn map_transaction_error(
    err: sea_orm::TransactionError<SwitchActiveAccountAtomicError>,
) -> SwitchActiveAccountAtomicError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => {
            SwitchActiveAccountAtomicError::Store(StoreError::from(db_err))
        }
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}

#[derive(Iden)]
enum ActiveAccountScopes {
    Table,
    ScopeHash,
    ActiveAccountId,
    SignedInAccountIds,
    UpdatedAt,
}
