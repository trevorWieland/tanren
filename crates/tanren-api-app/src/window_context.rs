use axum::http::HeaderMap;
use tanren_contract::{WindowContextId, WindowContextValidationError};

pub(crate) const WINDOW_ID_HEADER: &str = "x-tanren-window-id";

/// Normalized error code returned to clients when window-context
/// validation fails. Maps into the closed
/// `AccountFailureReason::ValidationFailed` taxonomy so that
/// unauthenticated callers receive a stable machine code without
/// gaining insight into internal header structure.
pub(crate) const WINDOW_CONTEXT_ERROR_CODE: &str = "validation_failed";

#[derive(Debug, Clone, Copy)]
pub(crate) enum WindowContextError {
    Missing,
    InvalidUtf8,
    Invalid(WindowContextValidationError),
}

impl WindowContextError {
    /// Detailed internal summary for structured logging. Never sent
    /// directly to unauthenticated callers.
    pub(crate) const fn summary(self) -> &'static str {
        match self {
            Self::Missing => "x-tanren-window-id header is required",
            Self::InvalidUtf8 => "window id must be valid UTF-8",
            Self::Invalid(WindowContextValidationError::Empty) => "window id must not be empty",
            Self::Invalid(WindowContextValidationError::TooLong) => {
                "window id must be 128 bytes or shorter"
            }
            Self::Invalid(WindowContextValidationError::InvalidUuid) => "window id must be a UUID",
        }
    }

    /// Client-safe summary that avoids leaking internal header
    /// structure. Used in responses to unauthenticated or
    /// partially-authenticated callers so they receive a stable
    /// human-readable message alongside the machine code.
    pub(crate) const fn client_summary(self) -> &'static str {
        match self {
            Self::Missing => "window context identifier is required",
            Self::InvalidUtf8 | Self::Invalid(_) => "window context identifier is invalid",
        }
    }
}

pub(crate) fn resolve_window_context(
    headers: &HeaderMap,
) -> Result<WindowContextId, WindowContextError> {
    let Some(value) = headers.get(WINDOW_ID_HEADER) else {
        return Err(WindowContextError::Missing);
    };
    let Ok(value) = value.to_str() else {
        return Err(WindowContextError::InvalidUtf8);
    };
    WindowContextId::parse(value).map_err(WindowContextError::Invalid)
}
