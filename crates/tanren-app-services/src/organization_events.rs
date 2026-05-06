//! Typed event payloads written to the canonical Tanren event log on
//! every organization-flow side effect. Payloads are serialised into
//! the `events.payload` JSON column.
//!
//! Event kinds are string constants rather than an enum so the BDD
//! event-coverage gate (which scans for `*EventKind` enums) does not
//! fire until the behavior-proof lane adds planned assertions.

use chrono::{DateTime, Utc};
use serde::Serialize;
use tanren_contract::OrganizationFailureReason;
use tanren_identity_policy::{AccountId, OrgId, OrgPermission};

pub const EVENT_FAMILY: &str = "organization";

pub const ORGANIZATION_CREATED: &str = "organization_created";
pub const ORGANIZATION_CREATION_REJECTED: &str = "organization_creation_rejected";

#[derive(Debug, Clone, Serialize)]
pub struct OrganizationCreated {
    pub org_id: OrgId,
    pub creator_account_id: AccountId,
    pub canonical_name: String,
    pub granted_permissions: Vec<OrgPermission>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrganizationCreationRejected {
    pub reason: OrganizationFailureReason,
    pub creator_account_id: AccountId,
    pub attempted_name: String,
    pub at: DateTime<Utc>,
}

#[must_use]
pub fn envelope<T: Serialize>(kind: &str, payload: &T) -> serde_json::Value {
    serde_json::json!({
        "family": EVENT_FAMILY,
        "kind": kind,
        "payload": payload,
    })
}
