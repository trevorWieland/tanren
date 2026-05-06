//! `SeaORM`-backed implementations of notification preference, org-override,
//! and pending-route CRUD operations, plus membership-lookup support for
//! authorization checks.

use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tanren_configuration_secrets::{NotificationChannelSet, NotificationEventType};
use tanren_identity_policy::{AccountId, OrgId};
use uuid::Uuid;

use crate::entity;
use crate::{
    NotificationOrgOverrideRecord, NotificationPreferenceRecord, PendingNotificationRouteRecord,
    StoreError,
};

pub(crate) async fn upsert_notification_preference(
    conn: &DatabaseConnection,
    account_id: AccountId,
    event_type: NotificationEventType,
    enabled_channels: NotificationChannelSet,
    now: DateTime<Utc>,
) -> Result<NotificationPreferenceRecord, StoreError> {
    let event_type_str = event_type.to_string();
    let channels_json =
        serde_json::to_string(&enabled_channels).map_err(|e| StoreError::Deserialization {
            entity: "notification_preferences",
            column: "enabled_channels",
            cause: e.to_string(),
        })?;

    let existing = entity::notification_preferences::Entity::find()
        .filter(entity::notification_preferences::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::notification_preferences::Column::EventType.eq(&event_type_str))
        .one(conn)
        .await?;

    let model = if let Some(row) = existing {
        let mut active: entity::notification_preferences::ActiveModel = row.into();
        active.enabled_channels = Set(channels_json);
        active.updated_at = Set(now);
        active.update(conn).await?
    } else {
        let model = entity::notification_preferences::ActiveModel {
            id: Set(Uuid::now_v7()),
            account_id: Set(account_id.as_uuid()),
            event_type: Set(event_type_str),
            enabled_channels: Set(channels_json),
            updated_at: Set(now),
        };
        model.insert(conn).await?
    };

    NotificationPreferenceRecord::try_from(model)
}

pub(crate) async fn list_notification_preferences(
    conn: &DatabaseConnection,
    account_id: AccountId,
) -> Result<Vec<NotificationPreferenceRecord>, StoreError> {
    let rows = entity::notification_preferences::Entity::find()
        .filter(entity::notification_preferences::Column::AccountId.eq(account_id.as_uuid()))
        .all(conn)
        .await?;
    rows.into_iter()
        .map(NotificationPreferenceRecord::try_from)
        .collect()
}

pub(crate) async fn upsert_notification_org_override(
    conn: &DatabaseConnection,
    account_id: AccountId,
    org_id: OrgId,
    event_type: NotificationEventType,
    enabled_channels: NotificationChannelSet,
    now: DateTime<Utc>,
) -> Result<NotificationOrgOverrideRecord, StoreError> {
    let event_type_str = event_type.to_string();
    let channels_json =
        serde_json::to_string(&enabled_channels).map_err(|e| StoreError::Deserialization {
            entity: "notification_org_overrides",
            column: "enabled_channels",
            cause: e.to_string(),
        })?;

    let existing = entity::notification_org_overrides::Entity::find()
        .filter(entity::notification_org_overrides::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::notification_org_overrides::Column::OrgId.eq(org_id.as_uuid()))
        .filter(entity::notification_org_overrides::Column::EventType.eq(&event_type_str))
        .one(conn)
        .await?;

    let model = if let Some(row) = existing {
        let mut active: entity::notification_org_overrides::ActiveModel = row.into();
        active.enabled_channels = Set(channels_json);
        active.updated_at = Set(now);
        active.update(conn).await?
    } else {
        let model = entity::notification_org_overrides::ActiveModel {
            id: Set(Uuid::now_v7()),
            account_id: Set(account_id.as_uuid()),
            org_id: Set(org_id.as_uuid()),
            event_type: Set(event_type_str),
            enabled_channels: Set(channels_json),
            updated_at: Set(now),
        };
        model.insert(conn).await?
    };

    NotificationOrgOverrideRecord::try_from(model)
}

pub(crate) async fn list_notification_org_overrides(
    conn: &DatabaseConnection,
    account_id: AccountId,
    org_id: OrgId,
) -> Result<Vec<NotificationOrgOverrideRecord>, StoreError> {
    let rows = entity::notification_org_overrides::Entity::find()
        .filter(entity::notification_org_overrides::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::notification_org_overrides::Column::OrgId.eq(org_id.as_uuid()))
        .all(conn)
        .await?;
    rows.into_iter()
        .map(NotificationOrgOverrideRecord::try_from)
        .collect()
}

pub(crate) async fn upsert_pending_notification_route(
    conn: &DatabaseConnection,
    account_id: AccountId,
    event_type: NotificationEventType,
    channels_snapshot: NotificationChannelSet,
    overriding_org_id: Option<OrgId>,
    now: DateTime<Utc>,
) -> Result<PendingNotificationRouteRecord, StoreError> {
    let event_type_str = event_type.to_string();
    let snapshot_json =
        serde_json::to_string(&channels_snapshot).map_err(|e| StoreError::Deserialization {
            entity: "pending_notification_routes",
            column: "channels_snapshot",
            cause: e.to_string(),
        })?;

    let existing = entity::pending_notification_routes::Entity::find()
        .filter(entity::pending_notification_routes::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::pending_notification_routes::Column::EventType.eq(&event_type_str))
        .one(conn)
        .await?;

    let model = if let Some(row) = existing {
        let mut active: entity::pending_notification_routes::ActiveModel = row.into();
        active.overriding_org_id = Set(overriding_org_id.map(OrgId::as_uuid));
        active.computed_at = Set(now);
        active.update(conn).await?
    } else {
        let model = entity::pending_notification_routes::ActiveModel {
            id: Set(Uuid::now_v7()),
            account_id: Set(account_id.as_uuid()),
            event_type: Set(event_type_str),
            channels_snapshot: Set(snapshot_json),
            overriding_org_id: Set(overriding_org_id.map(OrgId::as_uuid)),
            computed_at: Set(now),
        };
        model.insert(conn).await?
    };

    PendingNotificationRouteRecord::try_from(model)
}

pub(crate) async fn list_pending_notification_routes(
    conn: &DatabaseConnection,
    account_id: AccountId,
) -> Result<Vec<PendingNotificationRouteRecord>, StoreError> {
    let rows = entity::pending_notification_routes::Entity::find()
        .filter(entity::pending_notification_routes::Column::AccountId.eq(account_id.as_uuid()))
        .all(conn)
        .await?;
    rows.into_iter()
        .map(PendingNotificationRouteRecord::try_from)
        .collect()
}

pub(crate) async fn is_account_member_of_org(
    conn: &DatabaseConnection,
    account_id: AccountId,
    org_id: OrgId,
) -> Result<bool, StoreError> {
    let row = entity::memberships::Entity::find()
        .filter(entity::memberships::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::memberships::Column::OrgId.eq(org_id.as_uuid()))
        .one(conn)
        .await?;
    Ok(row.is_some())
}

pub(crate) fn notification_event_type_from_db(
    raw: &str,
) -> Result<NotificationEventType, StoreError> {
    match raw {
        "loop_completed" => Ok(NotificationEventType::LoopCompleted),
        "walk_requested" => Ok(NotificationEventType::WalkRequested),
        _ => Err(StoreError::Deserialization {
            entity: "notification_preferences",
            column: "event_type",
            cause: format!("unknown notification event type: {raw}"),
        }),
    }
}

pub(crate) fn notification_channels_from_db(
    entity_name: &'static str,
    raw: &str,
) -> Result<NotificationChannelSet, StoreError> {
    serde_json::from_str(raw).map_err(|e| StoreError::Deserialization {
        entity: entity_name,
        column: "enabled_channels",
        cause: e.to_string(),
    })
}
