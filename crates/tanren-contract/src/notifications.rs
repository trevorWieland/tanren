//! Notification preference command/response wire shapes.
//!
//! These types are the request/response surface used by the api, mcp,
//! cli, tui, and web client when callers manage notification preferences,
//! set organization-level overrides, evaluate routing decisions, or read
//! a pending routing snapshot. They live in `tanren-contract` because
//! every interface binary serialises the same shapes.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_configuration_secrets::{
    NotificationChannelSet, NotificationEventType, NotificationPreference,
};
use tanren_identity_policy::OrgId;
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// Set user notification preferences
// ---------------------------------------------------------------------------

/// Request to set (upsert) the current user's notification preferences
/// for one or more event types.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SetNotificationPreferencesRequest {
    /// Preferences to upsert. Existing preferences for the same event
    /// types are replaced; other event types are left unchanged.
    pub preferences: Vec<NotificationPreference>,
}

/// Successful response for setting notification preferences.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SetNotificationPreferencesResponse {
    /// All preferences after the upsert, including unchanged entries.
    pub preferences: Vec<NotificationPreference>,
}

// ---------------------------------------------------------------------------
// List notification preferences
// ---------------------------------------------------------------------------

/// Request to list the current user's notification preferences.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListNotificationPreferencesRequest {}

/// Successful response for listing notification preferences.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListNotificationPreferencesResponse {
    /// All user-level notification preferences currently set.
    pub preferences: Vec<NotificationPreference>,
}

// ---------------------------------------------------------------------------
// Organization notification overrides
// ---------------------------------------------------------------------------

/// Request to set (upsert) an organization-level notification override.
///
/// Organization overrides allow an org admin to mandate or suppress
/// notification channels for specific event types for all members of
/// the organization. User-level preferences are merged with org
/// overrides during route evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SetOrganizationNotificationOverridesRequest {
    /// Organization whose override is being set.
    pub org_id: OrgId,
    /// Overrides to upsert for this organization.
    pub overrides: Vec<OrganizationNotificationOverride>,
}

/// Successful response for setting organization notification overrides.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SetOrganizationNotificationOverridesResponse {
    /// All overrides for the organization after the upsert.
    pub overrides: Vec<OrganizationNotificationOverride>,
}

/// A single organization-level notification override.
///
/// When present, this override is merged with (and takes precedence
/// over) the user's own preference for the same event type during
/// route evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationNotificationOverride {
    /// The event type this override governs.
    pub event_type: NotificationEventType,
    /// Channels mandated (or suppressed) by the organization.
    pub enabled_channels: NotificationChannelSet,
}

// ---------------------------------------------------------------------------
// Evaluate notification route
// ---------------------------------------------------------------------------

/// Request to evaluate which channels an event would be routed through
/// for a given user and organization context.
///
/// Used by the notification delivery system (M-0021) to determine the
/// effective channel set after merging user preferences with
/// organization overrides.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct EvaluateNotificationRouteRequest {
    /// The event type being evaluated.
    pub event_type: NotificationEventType,
    /// Organization context for override resolution. `None` means
    /// no organization overrides apply.
    pub org_id: Option<OrgId>,
}

/// Successful response for notification route evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct EvaluateNotificationRouteResponse {
    /// The event type that was evaluated.
    pub event_type: NotificationEventType,
    /// Effective channels after merging user preferences with
    /// organization overrides.
    pub channels: NotificationChannelSet,
}

// ---------------------------------------------------------------------------
// Pending routing snapshot
// ---------------------------------------------------------------------------

/// Request to read the current pending routing snapshot for the
/// authenticated user.
///
/// The snapshot captures the fully-resolved routing table: user
/// preferences merged with all applicable organization overrides,
/// ready for consumption by the notification delivery system.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ReadPendingRoutingSnapshotRequest {}

/// A single entry in the pending routing snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RoutingSnapshotEntry {
    /// The event type this entry covers.
    pub event_type: NotificationEventType,
    /// Effective channels after all merges.
    pub channels: NotificationChannelSet,
    /// Organization whose override contributed to this entry, if any.
    pub overriding_org: Option<OrgId>,
}

/// Successful response for reading the pending routing snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ReadPendingRoutingSnapshotResponse {
    /// All resolved routing entries, one per event type with an
    /// active preference or override.
    pub entries: Vec<RoutingSnapshotEntry>,
    /// Wall-clock time at which this snapshot was computed.
    pub computed_at: DateTime<Utc>,
}
