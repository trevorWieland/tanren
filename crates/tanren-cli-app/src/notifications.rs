use std::io::Write;

use anyhow::{Context, Result};
use tanren_configuration_secrets::{
    NotificationChannel, NotificationChannelSet, NotificationEventType, NotificationPreference,
};
use tanren_contract::{
    EvaluateNotificationRouteRequest, OrganizationNotificationOverride,
    SetNotificationPreferencesRequest, SetOrganizationNotificationOverridesRequest,
};
use tanren_identity_policy::OrgId;
use uuid::Uuid;

use crate::commands::{app_service_error, connect_and_authenticate};
use crate::{
    NotificationAction, NotificationOrgOverrideAction, NotificationPrefAction,
    NotificationRouteAction,
};

pub(super) fn dispatch_notification(action: NotificationAction) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    rt.block_on(run_notification(action))
}

async fn run_notification(action: NotificationAction) -> Result<()> {
    match action {
        NotificationAction::Preferences { action } => run_preferences(action).await,
        NotificationAction::OrgOverride { action } => run_org_override(action).await,
        NotificationAction::Route { action } => run_route(action).await,
    }
}

async fn run_preferences(action: NotificationPrefAction) -> Result<()> {
    match action {
        NotificationPrefAction::Set {
            database_url,
            event_type,
            channels,
        } => {
            let (handlers, store, actor) = connect_and_authenticate(&database_url).await?;
            let event_type = parse_event_type(&event_type)?;
            let channels = parse_channels(&channels)?;
            let request = SetNotificationPreferencesRequest {
                preferences: vec![NotificationPreference {
                    event_type,
                    enabled_channels: channels,
                }],
            };
            let resp = handlers
                .set_notification_preferences(&store, &actor, request)
                .await
                .map_err(app_service_error)?;
            let mut h = std::io::stdout().lock();
            for p in &resp.preferences {
                writeln!(
                    h,
                    "event_type={} channels={}",
                    p.event_type,
                    format_channels(&p.enabled_channels)
                )
                .context("write preference")?;
            }
        }
        NotificationPrefAction::List { database_url } => {
            let (handlers, store, actor) = connect_and_authenticate(&database_url).await?;
            let resp = handlers
                .list_notification_preferences(&store, &actor)
                .await
                .map_err(app_service_error)?;
            let mut h = std::io::stdout().lock();
            for p in &resp.preferences {
                writeln!(
                    h,
                    "event_type={} channels={}",
                    p.event_type,
                    format_channels(&p.enabled_channels)
                )
                .context("write preference")?;
            }
        }
    }
    Ok(())
}

async fn run_org_override(action: NotificationOrgOverrideAction) -> Result<()> {
    match action {
        NotificationOrgOverrideAction::Set {
            database_url,
            org_id,
            event_type,
            channels,
        } => {
            let (handlers, store, actor) = connect_and_authenticate(&database_url).await?;
            let org_id = parse_org_id(&org_id)?;
            let event_type = parse_event_type(&event_type)?;
            let channels = parse_channels(&channels)?;
            let request = SetOrganizationNotificationOverridesRequest {
                org_id,
                overrides: vec![OrganizationNotificationOverride {
                    event_type,
                    enabled_channels: channels,
                }],
            };
            let resp = handlers
                .set_organization_notification_overrides(&store, &actor, request)
                .await
                .map_err(app_service_error)?;
            let mut h = std::io::stdout().lock();
            for o in &resp.overrides {
                writeln!(
                    h,
                    "org_id={} event_type={} channels={}",
                    org_id,
                    o.event_type,
                    format_channels(&o.enabled_channels)
                )
                .context("write org override")?;
            }
        }
    }
    Ok(())
}

async fn run_route(action: NotificationRouteAction) -> Result<()> {
    match action {
        NotificationRouteAction::Evaluate {
            database_url,
            event_type,
            org_id,
        } => {
            let (handlers, store, actor) = connect_and_authenticate(&database_url).await?;
            let event_type = parse_event_type(&event_type)?;
            let org_id = org_id.map(|s| parse_org_id(&s)).transpose()?;
            let request = EvaluateNotificationRouteRequest { event_type, org_id };
            let resp = handlers
                .evaluate_notification_route(&store, &actor, request)
                .await
                .map_err(app_service_error)?;
            writeln!(
                std::io::stdout().lock(),
                "event_type={} channels={}",
                resp.event_type,
                format_channels(&resp.channels)
            )
            .context("write evaluate result")?;
        }
    }
    Ok(())
}

fn parse_event_type(raw: &str) -> Result<NotificationEventType> {
    serde_json::from_value(serde_json::Value::String(raw.to_owned()))
        .with_context(|| format!("invalid event type: {raw}"))
}

fn parse_channels(raw: &str) -> Result<NotificationChannelSet> {
    let mut channels = Vec::new();
    for part in raw.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let ch: NotificationChannel =
            serde_json::from_value(serde_json::Value::String(trimmed.to_owned()))
                .with_context(|| format!("invalid channel: {trimmed}"))?;
        channels.push(ch);
    }
    Ok(channels.into_iter().collect())
}

fn parse_org_id(raw: &str) -> Result<OrgId> {
    Ok(OrgId::new(
        Uuid::parse_str(raw).context("parse org id as UUID")?,
    ))
}

fn format_channels(channels: &NotificationChannelSet) -> String {
    channels
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
