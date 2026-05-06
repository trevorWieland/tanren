//! Standards read-model — canonical effective configuration derived
//! from event-sourced state.
//!
//! The [`StandardsReadModel`] replays standards-family events to
//! determine the effective standards root for a repository. It is the
//! single source of truth for effective configuration — callers must
//! not read `tanren.yml` (or any other filesystem config) to resolve
//! the standards root. The config file may remain as an optional
//! drift/projection input for later delivery plumbing, but it is never
//! the canonical effective-configuration source.
//!
//! # Event → state projection
//!
//! | Event                        | Effect                              |
//! |------------------------------|-------------------------------------|
//! | `standards_root_configured`  | Sets effective root + schema        |
//! | `standards_root_cleared`     | Resets to "not configured"          |
//!
//! # Usage
//!
//! ```ignore
//! use tanren_app_services::standards::{StandardsReadModel, configure_standards_root};
//! use tanren_contract::StandardSchema;
//! use std::path::PathBuf;
//!
//! let mut rm = StandardsReadModel::default();
//! let event = configure_standards_root(
//!     PathBuf::from("/repo/standards"),
//!     StandardSchema::current(),
//!     chrono::Utc::now(),
//! );
//! rm.apply_event(&event);
//! assert!(rm.effective_root().is_ok());
//! ```

pub mod projection;

use std::path::{Path, PathBuf};

use tanren_contract::{StandardSchema, StandardsFailureReason};
use thiserror::Error;

use crate::events::{
    StandardsEventKind, StandardsRootCleared, StandardsRootConfigured, standards_envelope,
};

/// Errors raised when resolving the effective standards root from the
/// read-model.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum StandardsResolutionError {
    /// No `standards_root_configured` event has been applied, or a
    /// `standards_root_cleared` event was the last one applied.
    #[error("standards root not configured")]
    NotConfigured,
}

impl From<StandardsResolutionError> for StandardsFailureReason {
    fn from(value: StandardsResolutionError) -> Self {
        match value {
            StandardsResolutionError::NotConfigured => {
                StandardsFailureReason::StandardsRootNotFound
            }
        }
    }
}

/// Read-model that resolves the effective standards root from
/// event-sourced state.
///
/// This is the canonical source of truth for where installed standards
/// live. It is populated by replaying standards-family events (via
/// [`apply_event`](StandardsReadModel::apply_event)).
///
/// Interface binaries construct a `StandardsReadModel` by replaying
/// persisted events and then query [`effective_root`](StandardsReadModel::effective_root)
/// to locate installed standards. The model never reads `tanren.yml`
/// or any other filesystem config — effective configuration comes
/// exclusively from the event stream.
#[derive(Debug, Clone, Default)]
pub struct StandardsReadModel {
    effective_root: Option<PathBuf>,
    schema: Option<StandardSchema>,
}

impl StandardsReadModel {
    /// Apply a standards-family event envelope to the read-model.
    ///
    /// The envelope is expected to be a JSON object with `"family"`,
    /// `"kind"`, and `"payload"` fields as produced by
    /// [`standards_envelope`]. Non-standards envelopes are silently
    /// ignored.
    pub fn apply_event(&mut self, envelope: &serde_json::Value) {
        let family = envelope.get("family").and_then(|v| v.as_str());
        if family != Some(crate::events::STANDARDS_EVENT_FAMILY) {
            return;
        }
        let kind = envelope.get("kind").and_then(|v| v.as_str());
        match kind {
            Some("standards_root_configured") => {
                if let Ok(payload) =
                    serde_json::from_value::<StandardsRootConfigured>(envelope["payload"].clone())
                {
                    self.effective_root = Some(payload.standards_root);
                    self.schema = Some(payload.schema);
                }
            }
            Some("standards_root_cleared") => {
                if serde_json::from_value::<StandardsRootCleared>(envelope["payload"].clone())
                    .is_ok()
                {
                    self.effective_root = None;
                    self.schema = None;
                }
            }
            _ => {}
        }
    }

    /// Replay a batch of event envelopes, applying each in order.
    pub fn apply_events(&mut self, envelopes: &[serde_json::Value]) {
        for envelope in envelopes {
            self.apply_event(envelope);
        }
    }

    /// Resolve the effective standards root directory.
    ///
    /// # Errors
    ///
    /// Returns [`StandardsResolutionError::NotConfigured`] when no
    /// `standards_root_configured` event has been applied (or a
    /// `standards_root_cleared` event was the last one applied).
    pub fn effective_root(&self) -> Result<&Path, StandardsResolutionError> {
        self.effective_root
            .as_deref()
            .ok_or(StandardsResolutionError::NotConfigured)
    }

    /// The schema version of the current standards configuration, if
    /// any.
    #[must_use]
    pub fn schema(&self) -> Option<&StandardSchema> {
        self.schema.as_ref()
    }

    /// Whether a standards root has been configured.
    #[must_use]
    pub fn is_configured(&self) -> bool {
        self.effective_root.is_some()
    }
}

/// Build a `standards_root_configured` event envelope suitable for
/// persisting or replaying through [`StandardsReadModel::apply_event`].
#[must_use]
pub fn configure_standards_root(
    root: PathBuf,
    schema: StandardSchema,
    at: chrono::DateTime<chrono::Utc>,
) -> serde_json::Value {
    standards_envelope(
        StandardsEventKind::StandardsRootConfigured,
        &StandardsRootConfigured {
            standards_root: root,
            schema,
            at,
        },
    )
}

/// Build a `standards_root_cleared` event envelope suitable for
/// persisting or replaying through [`StandardsReadModel::apply_event`].
#[must_use]
pub fn clear_standards_root(at: chrono::DateTime<chrono::Utc>) -> serde_json::Value {
    standards_envelope(
        StandardsEventKind::StandardsRootCleared,
        &StandardsRootCleared { at },
    )
}
