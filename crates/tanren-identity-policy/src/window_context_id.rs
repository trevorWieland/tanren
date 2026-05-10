use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

const WINDOW_CONTEXT_ID_MAX_LEN: usize = 128;

/// Stable identifier for one browser-window context.
///
/// Used by the web/api active-account switch contract to scope the active
/// account inside one authenticated caller session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String, format = "uuid")]
pub struct WindowContextId(Uuid);

impl WindowContextId {
    /// Parse and validate a caller-supplied window-context identifier.
    ///
    /// # Errors
    ///
    /// Returns [`WindowContextValidationError`] when the value is blank,
    /// exceeds 128 bytes, or is not a UUID.
    pub fn parse(raw: &str) -> Result<Self, WindowContextValidationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(WindowContextValidationError::Empty);
        }
        if trimmed.len() > WINDOW_CONTEXT_ID_MAX_LEN {
            return Err(WindowContextValidationError::TooLong);
        }
        let parsed =
            Uuid::parse_str(trimmed).map_err(|_| WindowContextValidationError::InvalidUuid)?;
        Ok(Self(parsed))
    }
}

impl std::fmt::Display for WindowContextId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Validation failures for [`WindowContextId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WindowContextValidationError {
    /// The supplied value was blank.
    #[error("window id must not be empty")]
    Empty,
    /// The supplied value exceeded the bounded header size.
    #[error("window id must be 128 bytes or shorter")]
    TooLong,
    /// The supplied value was not a UUID.
    #[error("window id must be a UUID")]
    InvalidUuid,
}
