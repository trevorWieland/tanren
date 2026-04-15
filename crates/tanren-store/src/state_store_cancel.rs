use chrono::Utc;
use sea_orm::{
    ColumnTrait, DatabaseTransaction, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::Expr,
};
use tanren_domain::{
    ActorContext, DispatchId, DispatchStatus, DomainEvent, EntityKind, EventEnvelope, EventId,
    StepId, StepType,
};

use crate::converters::events as event_converters;
use crate::db_error_codes::{
    extract_db_error_code, is_postgres_contention_code, is_sqlite_contention_code,
};
use crate::entity::enums::{DispatchStatusModel, OutcomeModel, StepStatusModel, StepTypeModel};
use crate::entity::{dispatch_projection, events, step_projection};
use crate::errors::{StoreConflictClass, StoreError, StoreOperation};
use crate::params::ReplayGuard;
use crate::token_replay_store::consume_replay_guard_once;

const CANCEL_BATCH_SIZE: u64 = 500;

pub(crate) struct CancelDispatchTxnInput {
    pub(crate) dispatch_id: DispatchId,
    pub(crate) dispatch_uuid: uuid::Uuid,
    pub(crate) actor: ActorContext,
    pub(crate) reason: Option<String>,
    pub(crate) replay_guard: ReplayGuard,
    pub(crate) now: chrono::DateTime<Utc>,
    pub(crate) step_event_timestamp: chrono::DateTime<Utc>,
    pub(crate) dispatch_event_model: events::ActiveModel,
}

pub(crate) async fn run_cancel_dispatch_transaction(
    txn: &DatabaseTransaction,
    input: CancelDispatchTxnInput,
) -> Result<u64, StoreError> {
    let CancelDispatchTxnInput {
        dispatch_id,
        dispatch_uuid,
        actor,
        reason,
        replay_guard,
        now,
        step_event_timestamp,
        dispatch_event_model,
    } = input;

    consume_replay_guard_once(txn, replay_guard).await?;

    let current = fetch_dispatch_status_for_cancel(txn, dispatch_id, dispatch_uuid).await?;
    let cancelled_count = cancel_pending_steps_and_emit_events(
        txn,
        dispatch_id,
        dispatch_uuid,
        &actor,
        reason.as_ref(),
        now,
        step_event_timestamp,
    )
    .await?;

    let result = dispatch_projection::Entity::update_many()
        .col_expr(
            dispatch_projection::Column::Status,
            Expr::value(DispatchStatusModel::Cancelled),
        )
        .col_expr(
            dispatch_projection::Column::Outcome,
            Expr::value(Option::<OutcomeModel>::None),
        )
        .col_expr(dispatch_projection::Column::UpdatedAt, Expr::value(now))
        .filter(dispatch_projection::Column::DispatchId.eq(dispatch_uuid))
        .filter(dispatch_projection::Column::Status.eq(DispatchStatusModel::from(current)))
        .exec(txn)
        .await?;
    if result.rows_affected == 0 {
        return Err(StoreError::Conflict {
            class: StoreConflictClass::Contention,
            operation: StoreOperation::CancelDispatch,
            reason: format!("dispatch {dispatch_id} status changed concurrently from {current}"),
        });
    }

    events::Entity::insert(dispatch_event_model)
        .exec(txn)
        .await?;
    Ok(cancelled_count)
}

pub(crate) fn normalize_cancel_error(err: StoreError) -> StoreError {
    match err {
        StoreError::Database(db_err) => {
            classify_contention_db_error(&db_err).map_or(StoreError::Database(db_err), |class| {
                StoreError::Conflict {
                    class,
                    operation: StoreOperation::CancelDispatch,
                    reason: "dispatch cancellation contention".to_owned(),
                }
            })
        }
        other => other,
    }
}

async fn fetch_dispatch_status_for_cancel(
    txn: &DatabaseTransaction,
    dispatch_id: DispatchId,
    dispatch_uuid: uuid::Uuid,
) -> Result<DispatchStatus, StoreError> {
    let row = dispatch_projection::Entity::find_by_id(dispatch_uuid)
        .one(txn)
        .await?;
    let row = row.ok_or_else(|| StoreError::NotFound {
        entity_kind: EntityKind::Dispatch,
        id: dispatch_id.to_string(),
    })?;

    let current = DispatchStatus::from(row.status);
    if !current.can_transition_to(DispatchStatus::Cancelled) {
        return Err(StoreError::InvalidTransition {
            entity: format!("dispatch {dispatch_id}"),
            from: current.to_string(),
            to: DispatchStatus::Cancelled.to_string(),
        });
    }
    Ok(current)
}

async fn cancel_pending_steps_and_emit_events(
    txn: &DatabaseTransaction,
    dispatch_id: DispatchId,
    dispatch_uuid: uuid::Uuid,
    actor: &ActorContext,
    reason: Option<&String>,
    now: chrono::DateTime<Utc>,
    step_event_timestamp: chrono::DateTime<Utc>,
) -> Result<u64, StoreError> {
    let mut total_cancelled = 0_u64;

    loop {
        let rows: Vec<(uuid::Uuid, StepTypeModel)> = step_projection::Entity::find()
            .select_only()
            .column(step_projection::Column::StepId)
            .column(step_projection::Column::StepType)
            .filter(step_projection::Column::DispatchId.eq(dispatch_uuid))
            .filter(step_projection::Column::Status.eq(StepStatusModel::Pending))
            .filter(step_projection::Column::StepType.ne(StepTypeModel::Teardown))
            .order_by_asc(step_projection::Column::StepId)
            .limit(CANCEL_BATCH_SIZE)
            .into_tuple()
            .all(txn)
            .await?;
        if rows.is_empty() {
            break;
        }

        let step_ids: Vec<_> = rows.iter().map(|(step_id, _)| *step_id).collect();
        let cancelled = step_projection::Entity::update_many()
            .col_expr(
                step_projection::Column::Status,
                Expr::value(StepStatusModel::Cancelled),
            )
            .col_expr(step_projection::Column::UpdatedAt, Expr::value(now))
            .filter(step_projection::Column::DispatchId.eq(dispatch_uuid))
            .filter(step_projection::Column::StepId.is_in(step_ids))
            .filter(step_projection::Column::Status.eq(StepStatusModel::Pending))
            .filter(step_projection::Column::StepType.ne(StepTypeModel::Teardown))
            .exec(txn)
            .await?;

        let cancelled_count = cancelled.rows_affected;
        let expected_count = rows.len() as u64;
        if cancelled_count != expected_count {
            return Err(StoreError::Conflict {
                class: StoreConflictClass::Contention,
                operation: StoreOperation::CancelDispatch,
                reason: format!(
                    "step contention during cancel for dispatch {dispatch_id}: \
                     expected {expected_count} cancelled rows, observed {cancelled_count}"
                ),
            });
        }

        let mut events_to_insert = Vec::with_capacity(rows.len());
        for (step_id, step_type_raw) in rows {
            events_to_insert.push(mint_step_cancelled(
                dispatch_id,
                step_id,
                step_type_raw,
                actor,
                reason,
                step_event_timestamp,
            )?);
        }
        events::Entity::insert_many(events_to_insert)
            .exec(txn)
            .await?;
        total_cancelled += cancelled_count;
    }

    Ok(total_cancelled)
}

fn mint_step_cancelled(
    dispatch_id: DispatchId,
    step_id_uuid: uuid::Uuid,
    step_type_model: StepTypeModel,
    actor: &ActorContext,
    reason: Option<&String>,
    timestamp: chrono::DateTime<Utc>,
) -> Result<events::ActiveModel, StoreError> {
    let step_id = StepId::from_uuid(step_id_uuid);
    let step_type = StepType::from(step_type_model);
    let envelope = EventEnvelope::new(
        EventId::from_uuid(uuid::Uuid::now_v7()),
        timestamp,
        DomainEvent::StepCancelled {
            dispatch_id,
            step_id,
            step_type,
            caused_by: Some(actor.clone()),
            reason: reason.cloned(),
        },
    );
    event_converters::envelope_to_active_model(&envelope)
}

fn classify_contention_db_error(db_err: &DbErr) -> Option<StoreConflictClass> {
    let code = extract_db_error_code(db_err)?;
    if is_sqlite_contention_code(&code) || is_postgres_contention_code(&code) {
        Some(StoreConflictClass::Contention)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::DbErr;

    use super::normalize_cancel_error;
    use crate::StoreError;
    use crate::db_error_codes::{is_postgres_contention_code, is_sqlite_contention_code};

    #[test]
    fn sqlite_contention_codes_detect_busy_and_locked_classes() {
        assert!(
            is_sqlite_contention_code("5"),
            "SQLITE_BUSY should classify"
        );
        assert!(
            is_sqlite_contention_code("6"),
            "SQLITE_LOCKED should classify"
        );
        assert!(
            is_sqlite_contention_code("261"),
            "extended SQLITE_BUSY code should classify via primary code"
        );
        assert!(
            !is_sqlite_contention_code("2067"),
            "unique violation is not contention"
        );
    }

    #[test]
    fn postgres_contention_codes_detect_deadlock_and_serialization() {
        assert!(is_postgres_contention_code("40P01"));
        assert!(is_postgres_contention_code("40001"));
        assert!(is_postgres_contention_code("55P03"));
        assert!(!is_postgres_contention_code("23505"));
    }

    #[test]
    fn normalize_cancel_error_preserves_non_typed_database_errors() {
        let err = StoreError::Database(DbErr::Custom("connection refused".to_owned()));
        let normalized = normalize_cancel_error(err);
        assert!(
            matches!(normalized, StoreError::Database(_)),
            "expected database error, got {normalized:?}"
        );
    }
}
