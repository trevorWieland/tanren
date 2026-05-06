use std::collections::BTreeSet;

use tanren_configuration_secrets::{
    NotificationChannel, NotificationChannelSet, NotificationEventType, NotificationPreference,
};
use tanren_contract::{
    OrganizationNotificationOverride, SetNotificationPreferencesRequest,
    SetNotificationPreferencesResponse, SetOrganizationNotificationOverridesRequest,
    SetOrganizationNotificationOverridesResponse,
};
use tanren_identity_policy::OrgId;
use uuid::Uuid;

use crate::{FormField, FormState, OutcomeView};

pub(crate) fn notification_set_preference_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Event type (loop_completed / walk_requested)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Channels (comma-sep: visual,auditory,push)",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn notification_set_org_override_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Org ID (UUID)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Event type (loop_completed / walk_requested)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Channels (comma-sep: visual,auditory,push)",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn notification_set_preference_outcome(
    response: &SetNotificationPreferencesResponse,
) -> OutcomeView {
    let mut lines = Vec::new();
    for p in &response.preferences {
        lines.push(format!(
            "event_type={} channels={}",
            p.event_type,
            format_channels(&p.enabled_channels)
        ));
    }
    OutcomeView {
        title: "Notification preferences set",
        lines,
    }
}

pub(crate) fn notification_set_org_override_outcome(
    response: &SetOrganizationNotificationOverridesResponse,
) -> OutcomeView {
    let mut lines = Vec::new();
    for o in &response.overrides {
        lines.push(format!(
            "event_type={} channels={}",
            o.event_type,
            format_channels(&o.enabled_channels)
        ));
    }
    OutcomeView {
        title: "Org notification override set",
        lines,
    }
}

pub(crate) fn parse_notification_set_preference(
    state: &FormState,
) -> Result<SetNotificationPreferencesRequest, String> {
    let event_type = parse_event_type(state.value(0))?;
    let channels = parse_channels(state.value(1))?;
    Ok(SetNotificationPreferencesRequest {
        preferences: vec![NotificationPreference {
            event_type,
            enabled_channels: channels,
        }],
    })
}

pub(crate) fn parse_notification_set_org_override(
    state: &FormState,
) -> Result<SetOrganizationNotificationOverridesRequest, String> {
    let org_id = parse_org_id(state.value(0))?;
    let event_type = parse_event_type(state.value(1))?;
    let channels = parse_channels(state.value(2))?;
    Ok(SetOrganizationNotificationOverridesRequest {
        org_id,
        overrides: vec![OrganizationNotificationOverride {
            event_type,
            enabled_channels: channels,
        }],
    })
}

fn parse_event_type(raw: &str) -> Result<NotificationEventType, String> {
    match raw.trim() {
        "loop_completed" => Ok(NotificationEventType::LoopCompleted),
        "walk_requested" => Ok(NotificationEventType::WalkRequested),
        other => Err(format!("invalid event type: {other}")),
    }
}

fn parse_channels(raw: &str) -> Result<NotificationChannelSet, String> {
    let mut channels = BTreeSet::new();
    for part in raw.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let ch = match trimmed {
            "visual" => NotificationChannel::Visual,
            "auditory" => NotificationChannel::Auditory,
            "push" => NotificationChannel::Push,
            other => return Err(format!("invalid channel: {other}")),
        };
        channels.insert(ch);
    }
    Ok(channels.into_iter().collect())
}

fn parse_org_id(raw: &str) -> Result<OrgId, String> {
    Uuid::parse_str(raw.trim())
        .map(OrgId::new)
        .map_err(|e| format!("invalid org id: {e}"))
}

fn format_channels(channels: &NotificationChannelSet) -> String {
    channels
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
