use chrono::{DateTime, Utc};
use tanren_configuration_secrets::{
    NotificationChannelSet, NotificationEventType, NotificationPreference,
};
use tanren_contract::{
    ConfigurationFailureReason, EvaluateNotificationRouteRequest,
    EvaluateNotificationRouteResponse, ListNotificationPreferencesResponse,
    OrganizationNotificationOverride, ReadPendingRoutingSnapshotResponse, RoutingSnapshotEntry,
    SetNotificationPreferencesRequest, SetNotificationPreferencesResponse,
    SetOrganizationNotificationOverridesRequest, SetOrganizationNotificationOverridesResponse,
};
use tanren_identity_policy::{AccountId, OrgId};
use tanren_store::AccountStore;

use crate::events::{
    NOTIFICATION_ORG_OVERRIDE_REJECTED_KIND, NOTIFICATION_PREFERENCE_REJECTED_KIND,
    NotificationOrgOverrideRejected, NotificationPreferenceRejected, notification_envelope,
};
use crate::{AppServiceError, AuthenticatedActor, Clock};

pub(crate) async fn set_notification_preferences<S>(
    store: &S,
    clock: &Clock,
    actor: &AuthenticatedActor,
    supported_channels: &NotificationChannelSet,
    request: SetNotificationPreferencesRequest,
) -> Result<SetNotificationPreferencesResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let account_id = actor.account_id();

    for pref in &request.preferences {
        if let Some(_ch) = first_unsupported_channel(&pref.enabled_channels, supported_channels) {
            emit_preference_rejected(
                store,
                account_id,
                pref.event_type,
                ConfigurationFailureReason::UnsupportedNotificationChannel,
                now,
            )
            .await?;
            return Err(AppServiceError::Configuration(
                ConfigurationFailureReason::UnsupportedNotificationChannel,
            ));
        }
    }

    for pref in &request.preferences {
        store
            .upsert_notification_preference(
                account_id,
                pref.event_type,
                pref.enabled_channels.clone(),
                now,
            )
            .await?;
    }

    let records = store.list_notification_preferences(account_id).await?;
    Ok(SetNotificationPreferencesResponse {
        preferences: records.into_iter().map(preference_from_record).collect(),
    })
}

pub(crate) async fn list_notification_preferences<S>(
    store: &S,
    actor: &AuthenticatedActor,
) -> Result<ListNotificationPreferencesResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let records = store
        .list_notification_preferences(actor.account_id())
        .await?;
    Ok(ListNotificationPreferencesResponse {
        preferences: records.into_iter().map(preference_from_record).collect(),
    })
}

pub(crate) async fn set_organization_notification_overrides<S>(
    store: &S,
    clock: &Clock,
    actor: &AuthenticatedActor,
    supported_channels: &NotificationChannelSet,
    request: SetOrganizationNotificationOverridesRequest,
) -> Result<SetOrganizationNotificationOverridesResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let account_id = actor.account_id();

    if !store
        .is_account_member_of_org(account_id, request.org_id)
        .await?
    {
        emit_org_override_rejected(
            store,
            account_id,
            request.org_id,
            ConfigurationFailureReason::UnauthorizedOrganizationOverride,
            now,
        )
        .await?;
        return Err(AppServiceError::Configuration(
            ConfigurationFailureReason::UnauthorizedOrganizationOverride,
        ));
    }

    for ov in &request.overrides {
        if let Some(_ch) = first_unsupported_channel(&ov.enabled_channels, supported_channels) {
            emit_preference_rejected(
                store,
                account_id,
                ov.event_type,
                ConfigurationFailureReason::UnsupportedNotificationChannel,
                now,
            )
            .await?;
            return Err(AppServiceError::Configuration(
                ConfigurationFailureReason::UnsupportedNotificationChannel,
            ));
        }
    }

    for ov in &request.overrides {
        store
            .upsert_notification_org_override(
                account_id,
                request.org_id,
                ov.event_type,
                ov.enabled_channels.clone(),
                now,
            )
            .await?;
    }

    let records = store
        .list_notification_org_overrides(account_id, request.org_id)
        .await?;
    Ok(SetOrganizationNotificationOverridesResponse {
        overrides: records
            .into_iter()
            .map(|r| OrganizationNotificationOverride {
                event_type: r.event_type,
                enabled_channels: r.enabled_channels,
            })
            .collect(),
    })
}

pub(crate) async fn evaluate_notification_route<S>(
    store: &S,
    clock: &Clock,
    actor: &AuthenticatedActor,
    request: EvaluateNotificationRouteRequest,
) -> Result<EvaluateNotificationRouteResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let account_id = actor.account_id();
    let effective_channels =
        resolve_effective_channels(store, account_id, request.event_type, request.org_id).await?;

    store
        .upsert_pending_notification_route(
            account_id,
            request.event_type,
            effective_channels.clone(),
            request.org_id,
            now,
        )
        .await?;

    Ok(EvaluateNotificationRouteResponse {
        event_type: request.event_type,
        channels: effective_channels,
    })
}

pub(crate) async fn read_pending_routing_snapshot<S>(
    store: &S,
    clock: &Clock,
    actor: &AuthenticatedActor,
) -> Result<ReadPendingRoutingSnapshotResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let routes = store
        .list_pending_notification_routes(actor.account_id())
        .await?;

    let computed_at = routes
        .iter()
        .map(|r| r.computed_at)
        .max()
        .unwrap_or_else(|| clock.now());

    Ok(ReadPendingRoutingSnapshotResponse {
        entries: routes
            .into_iter()
            .map(|r| RoutingSnapshotEntry {
                event_type: r.event_type,
                channels: r.channels_snapshot,
                overriding_org: r.overriding_org_id,
            })
            .collect(),
        computed_at,
    })
}

async fn resolve_effective_channels<S>(
    store: &S,
    account_id: AccountId,
    event_type: NotificationEventType,
    org_id: Option<OrgId>,
) -> Result<NotificationChannelSet, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let prefs = store.list_notification_preferences(account_id).await?;
    let base_pref = prefs.iter().find(|p| p.event_type == event_type);

    if let Some(org_id) = org_id {
        let org_overrides = store
            .list_notification_org_overrides(account_id, org_id)
            .await?;
        let org_override = org_overrides.iter().find(|o| o.event_type == event_type);

        if let Some(ov) = org_override {
            return Ok(ov.enabled_channels.clone());
        }
    }

    Ok(base_pref.map_or(NotificationChannelSet::EMPTY, |p| {
        p.enabled_channels.clone()
    }))
}

fn preference_from_record(
    record: tanren_store::NotificationPreferenceRecord,
) -> NotificationPreference {
    NotificationPreference {
        event_type: record.event_type,
        enabled_channels: record.enabled_channels,
    }
}

fn first_unsupported_channel(
    requested: &NotificationChannelSet,
    supported: &NotificationChannelSet,
) -> Option<tanren_configuration_secrets::NotificationChannel> {
    requested.iter().find(|ch| !supported.contains(ch))
}

async fn emit_preference_rejected<S>(
    store: &S,
    account_id: AccountId,
    event_type: NotificationEventType,
    reason: ConfigurationFailureReason,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            notification_envelope(
                NOTIFICATION_PREFERENCE_REJECTED_KIND,
                &NotificationPreferenceRejected {
                    account_id,
                    event_type,
                    reason,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

async fn emit_org_override_rejected<S>(
    store: &S,
    account_id: AccountId,
    org_id: OrgId,
    reason: ConfigurationFailureReason,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            notification_envelope(
                NOTIFICATION_ORG_OVERRIDE_REJECTED_KIND,
                &NotificationOrgOverrideRejected {
                    account_id,
                    org_id,
                    reason,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}
