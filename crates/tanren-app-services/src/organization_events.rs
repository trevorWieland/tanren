//! Typed event payloads written to the canonical Tanren event log on
//! every organization-flow side effect. Payloads are serialised into
//! the `events.payload` JSON column.
//!
//! Event kinds, payload structs, and the list-organizations response
//! are defined in [`tanren_contract`] so every interface binary shares
//! the same wire shapes. This module re-exports them for convenience
//! within the app-service layer and provides the JSON-envelope helper.

pub use tanren_contract::{
    OrganizationCreatedEvent, OrganizationCreationRejectedEvent, OrganizationEventKind,
};

use serde::Serialize;

pub const EVENT_FAMILY: &str = "organization";

#[must_use]
pub fn envelope<T: Serialize>(kind: OrganizationEventKind, payload: &T) -> serde_json::Value {
    serde_json::json!({
        "family": EVENT_FAMILY,
        "kind": kind.as_str(),
        "payload": payload,
    })
}
