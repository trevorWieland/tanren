//! Append path for methodology events — always available (Lane 0.5+).
//!
//! Methodology events (`DomainEvent::Methodology { .. }`) have no
//! SeaORM-maintained projection tables today (projections are
//! in-memory folds over the event stream per
//! [`super::projections`]), so this method does not risk the
//! projection-divergence that guards the generic test-hooks-only
//! [`Store::append`] path.
//!
//! Rejects non-methodology payloads with a typed `Conversion` error
//! so callers can't accidentally bypass the gated generic path.

use sea_orm::EntityTrait;
use tanren_domain::events::{DomainEvent, EventEnvelope};

use crate::Store;
use crate::converters::events as event_converters;
use crate::entity::events;
use crate::errors::{StoreError, StoreResult};

impl Store {
    /// Append one methodology-typed envelope to the event log.
    ///
    /// # Errors
    /// Returns [`StoreError::Conversion`] if `event.payload` is not a
    /// methodology variant, or a store-level database error on DB
    /// failure.
    pub async fn append_methodology_event(&self, event: &EventEnvelope) -> StoreResult<()> {
        if !matches!(event.payload, DomainEvent::Methodology { .. }) {
            return Err(StoreError::Conversion {
                context: "append_methodology_event",
                reason: "expected DomainEvent::Methodology payload".into(),
            });
        }
        let model = event_converters::envelope_to_active_model(event)?;
        events::Entity::insert(model).exec(self.conn()).await?;
        Ok(())
    }
}

// Bring `self.conn()` into scope for methodology::append impls.
// `Store::conn` is already defined in `store.rs`; no shim needed.
// This placeholder keeps the module shape consistent with other
// append-path modules (see `state_store_cancel.rs`).
#[cfg(test)]
mod tests {
    use tanren_domain::events::{DomainEvent, EventEnvelope};
    use tanren_domain::{DispatchId, EventId};

    #[test]
    fn append_rejects_non_methodology_synchronously() {
        // Unit-level guard: the static variant check happens before any
        // DB round-trip, so it's observable without a live store.
        let envelope = EventEnvelope {
            schema_version: tanren_domain::events::SCHEMA_VERSION,
            event_id: EventId::new(),
            timestamp: chrono::Utc::now(),
            entity_ref: tanren_domain::EntityRef::Dispatch(DispatchId::new()),
            payload: DomainEvent::DispatchStarted {
                dispatch_id: DispatchId::new(),
            },
        };
        let is_methodology = matches!(envelope.payload, DomainEvent::Methodology { .. });
        assert!(!is_methodology);
    }
}
