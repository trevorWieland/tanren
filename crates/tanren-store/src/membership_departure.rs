//! `SeaORM`-backed implementation of the atomic membership-departure
//! flow. Wraps membership deletion and event appending in one DB
//! transaction so failure on any step rolls the whole flow back.

use sea_orm::{
    ActiveModelTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::entity;
use crate::traits::{
    DepartMemberAtomicOutput, DepartMemberAtomicRequest, DepartMemberError,
    DepartMemberEventContext, DepartMemberEventsBuilder,
};
use crate::{MembershipRecord, StoreError};

pub(crate) async fn run(
    conn: &DatabaseConnection,
    request: DepartMemberAtomicRequest,
) -> Result<DepartMemberAtomicOutput, DepartMemberError> {
    conn.transaction::<_, DepartMemberAtomicOutput, DepartMemberError>(|txn| {
        Box::pin(async move { run_in_txn(txn, request).await })
    })
    .await
    .map_err(map_transaction_error)
}

async fn run_in_txn(
    txn: &DatabaseTransaction,
    request: DepartMemberAtomicRequest,
) -> Result<DepartMemberAtomicOutput, DepartMemberError> {
    let DepartMemberAtomicRequest {
        membership_id,
        account_id,
        org_id,
        now,
        events_builder,
    } = request;

    let row = entity::memberships::Entity::find_by_id(membership_id.as_uuid())
        .one(txn)
        .await
        .map_err(StoreError::from)?
        .ok_or(DepartMemberError::MembershipNotFound)?;

    let deleted = MembershipRecord::try_from(row).map_err(DepartMemberError::Store)?;

    entity::memberships::Entity::delete_by_id(membership_id.as_uuid())
        .exec(txn)
        .await
        .map_err(StoreError::from)?;

    append_success_events_in_txn(
        txn,
        events_builder,
        &DepartMemberEventContext {
            account_id,
            org_id,
            now,
        },
    )
    .await?;

    Ok(DepartMemberAtomicOutput {
        deleted_membership: deleted,
    })
}

async fn append_success_events_in_txn(
    txn: &DatabaseTransaction,
    events_builder: DepartMemberEventsBuilder,
    ctx: &DepartMemberEventContext,
) -> Result<(), DepartMemberError> {
    for payload in (events_builder)(ctx) {
        let model = entity::events::ActiveModel {
            id: Set(Uuid::now_v7()),
            occurred_at: Set(ctx.now),
            payload: Set(payload),
        };
        model.insert(txn).await.map_err(StoreError::from)?;
    }
    Ok(())
}

fn map_transaction_error(err: sea_orm::TransactionError<DepartMemberError>) -> DepartMemberError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => {
            DepartMemberError::Store(StoreError::from(db_err))
        }
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}
