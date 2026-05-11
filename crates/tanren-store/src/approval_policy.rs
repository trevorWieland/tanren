//! `SeaORM`-backed implementation of approval-policy CRUD operations.
//!
//! Each mutation wraps both the row change and the canonical event
//! append in one database transaction so approval state and the event
//! log can never diverge. Invalid data (e.g. `required_approvals`
//! that overflows `i16`) is rejected rather than silently defaulted.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use tanren_identity_policy::{ApprovalPolicyId, GatedAction, OrgId};
use uuid::Uuid;

use crate::entity;
use crate::traits::{ApprovalPolicyError, ApprovalPolicyPage, ListApprovalPoliciesRequest};
use crate::{ApprovalPolicyRecord, StoreError};

/// Event envelope constants for approval-policy events.
///
/// Follows the canonical envelope shape defined in the State Architecture
/// (`docs/architecture/subsystems/state.md` § Event Log). Every required
/// envelope field is carried in the JSON payload so the event is
/// self-describing: replay, visibility filtering, idempotency checking,
/// and cross-subsystem auditing can proceed without deserialising into
/// subsystem-specific types. The `events` table columns supply `event_id`
/// (`UUIDv7`) and `occurred_at` as dedicated columns; these values are
/// also mirrored in the JSON payload for envelope self-containment.
///
/// The stable namespaced `event_type` follows the `{family}.{kind}`
/// convention so log consumers can filter by family or by exact type.
pub(crate) const EVENT_FAMILY: &str = "identity_policy";
pub(crate) const EVENT_KIND_POLICY_CREATED: &str = "approval_policy_created";
pub(crate) const EVENT_KIND_POLICY_UPDATED: &str = "approval_policy_updated";
pub(crate) const EVENT_KIND_POLICY_DELETED: &str = "approval_policy_deleted";
/// Stable namespaced event types per the canonical envelope contract
/// (`docs/architecture/subsystems/state.md` § Event Log). Each
/// `event_type` is the composite `"{family}.{kind}"` so log consumers
/// can filter by family or by exact type without parsing.
pub(crate) const EVENT_TYPE_POLICY_CREATED: &str = "identity_policy.approval_policy_created";
pub(crate) const EVENT_TYPE_POLICY_UPDATED: &str = "identity_policy.approval_policy_updated";
pub(crate) const EVENT_TYPE_POLICY_DELETED: &str = "identity_policy.approval_policy_deleted";
/// Envelope schema version — bump when the envelope structure changes
/// in a non-backward-compatible way.
pub(crate) const EVENT_SCHEMA_VERSION: u32 = 1;

/// Convert a `u16` `required_approvals` to the storage `i16`. Rejects
/// zero and overflow — these are security-sensitive invariants that
/// must not be silently weakened.
fn required_approvals_to_i16(value: u16) -> Result<i16, ApprovalPolicyError> {
    if value == 0 {
        return Err(ApprovalPolicyError::Store(StoreError::DataInvariant {
            column: "required_approvals",
            cause: tanren_identity_policy::ValidationError::GatedActionEmpty,
        }));
    }
    i16::try_from(value).map_err(|_| {
        ApprovalPolicyError::Store(StoreError::DataInvariant {
            column: "required_approvals",
            cause: tanren_identity_policy::ValidationError::GatedActionEmpty,
        })
    })
}

pub(crate) async fn create_approval_policy_atomic(
    conn: &DatabaseConnection,
    policy_id: ApprovalPolicyId,
    org_id: OrgId,
    gated_action: &GatedAction,
    required_approvals: u16,
    permitted_approver_permission: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<ApprovalPolicyRecord, ApprovalPolicyError> {
    let approvals_i16 = required_approvals_to_i16(required_approvals)?;
    let gated_action_owned = gated_action.as_str().to_owned();
    let approver_perm_owned = permitted_approver_permission.to_owned();
    conn.transaction::<_, ApprovalPolicyRecord, ApprovalPolicyError>(|txn| {
        Box::pin(async move {
            let model = entity::approval_policies::ActiveModel {
                id: Set(policy_id.as_uuid()),
                org_id: Set(org_id.as_uuid()),
                gated_action: Set(gated_action_owned),
                required_approvals: Set(approvals_i16),
                permitted_approver_permission: Set(approver_perm_owned),
                version: Set(1),
                created_at: Set(now),
                updated_at: Set(now),
            };
            let inserted = model.insert(txn).await.map_err(|err| {
                if is_unique_violation(&err, "idx_approval_policies_org_action_unique") {
                    ApprovalPolicyError::IdempotencyConflict
                } else {
                    ApprovalPolicyError::Store(StoreError::from(err))
                }
            })?;
            let record =
                ApprovalPolicyRecord::try_from(inserted).map_err(ApprovalPolicyError::Store)?;
            append_event_in_txn(
                txn,
                &record,
                EVENT_TYPE_POLICY_CREATED,
                EVENT_KIND_POLICY_CREATED,
                now,
            )
            .await?;
            Ok(record)
        })
    })
    .await
    .map_err(map_transaction_error)
}

pub(crate) async fn update_approval_policy_atomic(
    conn: &DatabaseConnection,
    policy_id: ApprovalPolicyId,
    org_id: OrgId,
    required_approvals: u16,
    permitted_approver_permission: &str,
    expected_version: u16,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<ApprovalPolicyRecord, ApprovalPolicyError> {
    let approvals_i16 = required_approvals_to_i16(required_approvals)?;
    let approver_perm_owned = permitted_approver_permission.to_owned();
    conn.transaction::<_, ApprovalPolicyRecord, ApprovalPolicyError>(|txn| {
        Box::pin(async move {
            let row = entity::approval_policies::Entity::find_by_id(policy_id.as_uuid())
                .one(txn)
                .await
                .map_err(|e| ApprovalPolicyError::Store(StoreError::from(e)))?
                .ok_or(ApprovalPolicyError::NotFound)?;

            let current_version = u16::try_from(row.version).map_err(|_| {
                ApprovalPolicyError::Store(StoreError::DataInvariant {
                    column: "version",
                    cause: tanren_identity_policy::ValidationError::GatedActionEmpty,
                })
            })?;
            if current_version != expected_version {
                return Err(ApprovalPolicyError::VersionConflict {
                    expected: expected_version,
                    actual: current_version,
                });
            }
            if row.org_id != org_id.as_uuid() {
                return Err(ApprovalPolicyError::NotFound);
            }

            let next_version = expected_version.saturating_add(1);
            let next_version_i16 = i16::try_from(next_version).map_err(|_| {
                ApprovalPolicyError::Store(StoreError::DataInvariant {
                    column: "version",
                    cause: tanren_identity_policy::ValidationError::GatedActionEmpty,
                })
            })?;
            let mut active: entity::approval_policies::ActiveModel = row.into();
            active.required_approvals = Set(approvals_i16);
            active.permitted_approver_permission = Set(approver_perm_owned);
            active.version = Set(next_version_i16);
            active.updated_at = Set(now);
            let updated = active
                .update(txn)
                .await
                .map_err(|e| ApprovalPolicyError::Store(StoreError::from(e)))?;
            let record =
                ApprovalPolicyRecord::try_from(updated).map_err(ApprovalPolicyError::Store)?;
            append_event_in_txn(
                txn,
                &record,
                EVENT_TYPE_POLICY_UPDATED,
                EVENT_KIND_POLICY_UPDATED,
                now,
            )
            .await?;
            Ok(record)
        })
    })
    .await
    .map_err(map_transaction_error)
}

pub(crate) async fn delete_approval_policy(
    conn: &DatabaseConnection,
    policy_id: ApprovalPolicyId,
    org_id: OrgId,
    expected_version: u16,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), ApprovalPolicyError> {
    conn.transaction::<_, (), ApprovalPolicyError>(|txn| {
        Box::pin(async move {
            let row = entity::approval_policies::Entity::find_by_id(policy_id.as_uuid())
                .one(txn)
                .await
                .map_err(|e| ApprovalPolicyError::Store(StoreError::from(e)))?
                .ok_or(ApprovalPolicyError::NotFound)?;

            let current_version = u16::try_from(row.version).map_err(|_| {
                ApprovalPolicyError::Store(StoreError::DataInvariant {
                    column: "version",
                    cause: tanren_identity_policy::ValidationError::GatedActionEmpty,
                })
            })?;
            if current_version != expected_version {
                return Err(ApprovalPolicyError::VersionConflict {
                    expected: expected_version,
                    actual: current_version,
                });
            }
            if row.org_id != org_id.as_uuid() {
                return Err(ApprovalPolicyError::NotFound);
            }

            entity::approval_policies::Entity::delete_by_id(policy_id.as_uuid())
                .exec(txn)
                .await
                .map_err(|e| ApprovalPolicyError::Store(StoreError::from(e)))?;

            let record = ApprovalPolicyRecord::try_from(row).map_err(ApprovalPolicyError::Store)?;
            append_event_in_txn(
                txn,
                &record,
                EVENT_TYPE_POLICY_DELETED,
                EVENT_KIND_POLICY_DELETED,
                now,
            )
            .await?;
            Ok(())
        })
    })
    .await
    .map_err(map_transaction_error)
}

pub(crate) async fn find_required_approval_for_action(
    conn: &DatabaseConnection,
    org_id: OrgId,
    action: &GatedAction,
) -> Result<Option<ApprovalPolicyRecord>, StoreError> {
    let row = entity::approval_policies::Entity::find()
        .filter(entity::approval_policies::Column::OrgId.eq(org_id.as_uuid()))
        .filter(entity::approval_policies::Column::GatedAction.eq(action.as_str()))
        .one(conn)
        .await?;
    row.map(ApprovalPolicyRecord::try_from).transpose()
}

pub(crate) async fn list_approval_policies(
    conn: &DatabaseConnection,
    request: ListApprovalPoliciesRequest,
) -> Result<ApprovalPolicyPage, StoreError> {
    let page_limit = request.limit.max(1);
    let mut query = entity::approval_policies::Entity::find()
        .filter(entity::approval_policies::Column::OrgId.eq(request.org_id.as_uuid()))
        .order_by_asc(entity::approval_policies::Column::CreatedAt)
        .limit(page_limit.saturating_add(1));
    if let Some(after) = request.cursor {
        query = query.filter(entity::approval_policies::Column::Id.gt(after.as_uuid()));
    }
    let mut rows = query.all(conn).await?;
    let mut next_cursor = None;
    let fetch_limit = usize::try_from(page_limit).unwrap_or(usize::MAX);
    if rows.len() > fetch_limit {
        rows.truncate(fetch_limit);
        if let Some(last) = rows.last() {
            next_cursor = Some(ApprovalPolicyId::new(last.id));
        }
    }
    let policies = rows
        .into_iter()
        .map(ApprovalPolicyRecord::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ApprovalPolicyPage {
        policies,
        next_cursor,
    })
}

/// Append a single canonical event envelope inside an ongoing transaction.
///
/// The persisted JSON carries the full envelope shape mandated by the
/// State Architecture § Event Log. Every required envelope field is
/// present so replay, visibility filtering, idempotency checking, and
/// cross-subsystem auditing can proceed without deserialising into
/// subsystem-specific types:
///
/// ```json
/// {
///   "event_id": "0192-...",
///   "event_type": "identity_policy.approval_policy_created",
///   "schema_version": 1,
///   "occurred_at": "2026-05-11T...",
///   "actor": null,
///   "scope": "organization",
///   "resource": "approval_policy:<uuid>",
///   "correlation_id": null,
///   "causation_id": null,
///   "idempotency_key": null,
///   "visibility": "internal",
///   "payload": { ... typed domain data ... }
/// }
/// ```
///
/// The `events` table columns supply `event_id` (`UUIDv7`) and
/// `occurred_at` as dedicated columns; these values are also mirrored
/// in the JSON payload for envelope self-containment. Fields not yet
/// available at the store layer (`actor`, `correlation_id`,
/// `causation_id`, `idempotency_key`) are set to `null`; the
/// app-service layer fills them when it provides the event builder
/// closure. `position` is absent because the `events` table does not
/// yet carry a global-position column — a future migration will add
/// it and backfill.
async fn append_event_in_txn(
    txn: &DatabaseTransaction,
    record: &ApprovalPolicyRecord,
    event_type: &str,
    kind: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), StoreError> {
    let event_id = Uuid::now_v7();
    let envelope = serde_json::json!({
        "event_id": event_id.to_string(),
        "event_type": event_type,
        "family": EVENT_FAMILY,
        "kind": kind,
        "schema_version": EVENT_SCHEMA_VERSION,
        "occurred_at": now.to_rfc3339(),
        "actor": null,
        "scope": "organization",
        "resource": format!("approval_policy:{}", record.id),
        "correlation_id": null,
        "causation_id": null,
        "idempotency_key": null,
        "visibility": "internal",
        "payload": {
            "policy_id": record.id.to_string(),
            "org_id": record.org_id.to_string(),
            "gated_action": record.gated_action.as_str(),
            "required_approvals": record.required_approvals,
            "permitted_approver_permission": record.permitted_approver_permission,
            "version": record.version,
        },
    });
    let model = entity::events::ActiveModel {
        id: Set(event_id),
        occurred_at: Set(now),
        payload: Set(envelope),
    };
    model.insert(txn).await?;
    Ok(())
}

fn is_unique_violation(err: &sea_orm::DbErr, constraint_name: &str) -> bool {
    let Some(sea_orm::SqlErr::UniqueConstraintViolation(details)) = err.sql_err() else {
        return false;
    };
    details.contains(constraint_name)
}

/// Translate the `SeaORM` transaction error into the domain error.
fn map_transaction_error(
    err: sea_orm::TransactionError<ApprovalPolicyError>,
) -> ApprovalPolicyError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => {
            ApprovalPolicyError::Store(StoreError::from(db_err))
        }
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}
