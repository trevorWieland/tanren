use axum::http::HeaderMap;
use tanren_contract::{WindowContextId, WindowContextValidationError};

pub(crate) const WINDOW_ID_HEADER: &str = "x-tanren-window-id";

#[derive(Debug, Clone, Copy)]
pub(crate) enum WindowContextError {
    Missing,
    InvalidUtf8,
    Invalid(WindowContextValidationError),
}

impl WindowContextError {
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
