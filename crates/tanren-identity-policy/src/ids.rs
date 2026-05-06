//! Project-scoped identifier newtypes.
//!
//! Follows the same pattern as `AccountId`, `OrgId`, and `MembershipId`
//! defined in `lib.rs` — opaque `UUIDv7` wrappers with `serde(transparent)`,
//! `JsonSchema`, `ToSchema`, and `Display` support.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

macro_rules! id_newtype {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema,
        )]
        #[serde(transparent)]
        #[schema(value_type = String, format = "uuid")]
        pub struct $name(Uuid);

        impl $name {
            /// Wrap a raw UUID.
            #[must_use]
            pub const fn new(value: Uuid) -> Self {
                Self(value)
            }

            /// Allocate a fresh time-ordered id.
            #[must_use]
            pub fn fresh() -> Self {
                Self(Uuid::now_v7())
            }

            /// The underlying UUID.
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }

        impl AsRef<Uuid> for $name {
            fn as_ref(&self) -> &Uuid {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_newtype!(ProjectId, "Stable identifier for a Tanren project.");
id_newtype!(SpecId, "Stable identifier for a Tanren spec.");
id_newtype!(LoopId, "Stable identifier for a Tanren loop.");
id_newtype!(MilestoneId, "Stable identifier for a Tanren milestone.");
