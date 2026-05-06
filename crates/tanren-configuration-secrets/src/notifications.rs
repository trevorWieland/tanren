//! Notification preference domain types.
//!
//! Typed enums for event types, delivery channels, and per-event
//! preference records. These types define the domain vocabulary shared
//! between the configuration-secrets crate and the contract layer.
//! Persistence, handlers, and interface routes live elsewhere.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Closed set of notification event types that users can subscribe to.
///
/// Each variant names a discrete event in the Tanren lifecycle. The set
/// is intentionally small; new event types are added as the notification
/// system grows.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NotificationEventType {
    /// A loop has completed execution.
    LoopCompleted,
    /// A walk (interactive exploration) has been requested.
    WalkRequested,
}

impl NotificationEventType {
    /// All known event types.
    pub fn all() -> &'static [Self] {
        &[Self::LoopCompleted, Self::WalkRequested]
    }
}

impl std::fmt::Display for NotificationEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoopCompleted => f.write_str("loop_completed"),
            Self::WalkRequested => f.write_str("walk_requested"),
        }
    }
}

/// Closed set of notification delivery channels.
///
/// Each variant names a discrete channel through which a notification
/// can be delivered to the user.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NotificationChannel {
    /// Visual (in-app / toast / badge) notification.
    Visual,
    /// Auditory (sound / beep) notification.
    Auditory,
    /// Push notification to a mobile or desktop device.
    Push,
}

impl NotificationChannel {
    /// All known delivery channels.
    pub fn all() -> &'static [Self] {
        &[Self::Visual, Self::Auditory, Self::Push]
    }
}

impl std::fmt::Display for NotificationChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Visual => f.write_str("visual"),
            Self::Auditory => f.write_str("auditory"),
            Self::Push => f.write_str("push"),
        }
    }
}

/// Deterministic sorted, deduplicated set of notification channels.
///
/// Wraps a [`BTreeSet`] so that iteration order is always the same
/// (ascending [`NotificationChannel`] ordinal) and duplicate channels
/// are silently collapsed. Serialises as a JSON array in sorted order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct NotificationChannelSet(BTreeSet<NotificationChannel>);

impl NotificationChannelSet {
    /// Empty channel set.
    pub const EMPTY: Self = Self(BTreeSet::new());

    /// Whether the set contains no channels.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Number of distinct channels in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether `channel` is present in the set.
    #[must_use]
    pub fn contains(&self, channel: &NotificationChannel) -> bool {
        self.0.contains(channel)
    }

    /// Iterate over the channels in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = NotificationChannel> + '_ {
        self.0.iter().copied()
    }
}

impl FromIterator<NotificationChannel> for NotificationChannelSet {
    fn from_iter<I: IntoIterator<Item = NotificationChannel>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Per-event-type notification preference.
///
/// Records which channels are enabled for a single event type. The
/// absence of a preference for an event type means "use the default"
/// (defined by the application service layer, not here).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct NotificationPreference {
    /// The event type this preference governs.
    pub event_type: NotificationEventType,
    /// Channels enabled for this event type.
    pub enabled_channels: NotificationChannelSet,
}
