//! Strongly-typed identifier newtypes wrapping [`uuid::Uuid`] v7.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generates a newtype ID wrapper around [`Uuid`].
///
/// Each generated type gets `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`,
/// `Hash`, `Serialize`, `Deserialize`, transparent serde, `Display`
/// delegating to the inner UUID, `new()` generating v7, and `from_uuid()`.
macro_rules! define_id {
    ($($(#[doc = $doc:expr])* $name:ident),+ $(,)?) => {
        $(
            $(#[doc = $doc])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
            #[serde(transparent)]
            pub struct $name(Uuid);

            impl $name {
                /// Create a new time-ordered (v7) identifier.
                #[must_use]
                pub fn new() -> Self {
                    Self(Uuid::now_v7())
                }

                /// Wrap an existing [`Uuid`].
                #[must_use]
                pub const fn from_uuid(uuid: Uuid) -> Self {
                    Self(uuid)
                }

                /// Return the inner [`Uuid`].
                #[must_use]
                pub const fn into_uuid(self) -> Uuid {
                    self.0
                }
            }

            impl Default for $name {
                fn default() -> Self {
                    Self::new()
                }
            }

            impl std::fmt::Display for $name {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    self.0.fmt(f)
                }
            }
        )+
    };
}

define_id!(
    /// Identifies a dispatch (top-level orchestration unit).
    DispatchId,
    /// Identifies a step within a dispatch.
    StepId,
    /// Identifies an execution lease.
    LeaseId,
    /// Identifies a user.
    UserId,
    /// Identifies a team.
    TeamId,
    /// Identifies an organization (top-level multi-tenant boundary).
    OrgId,
    /// Identifies an API key.
    ApiKeyId,
    /// Identifies a project.
    ProjectId,
    /// Identifies a domain event.
    EventId,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_ids_are_unique() {
        let a = DispatchId::new();
        let b = DispatchId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn from_uuid_roundtrip() {
        let uuid = Uuid::now_v7();
        let id = StepId::from_uuid(uuid);
        assert_eq!(id.into_uuid(), uuid);
    }

    #[test]
    fn display_delegates_to_uuid() {
        let uuid = Uuid::now_v7();
        let id = LeaseId::from_uuid(uuid);
        assert_eq!(id.to_string(), uuid.to_string());
    }

    #[test]
    fn serde_transparent_roundtrip() {
        let id = EventId::new();
        let json = serde_json::to_string(&id).expect("serialize");
        let back: EventId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, back);
    }
}
