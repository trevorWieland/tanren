use axum::http::HeaderMap;
use uuid::Uuid;

pub(crate) const WINDOW_ID_HEADER: &str = "x-tanren-window-id";
const WINDOW_ID_MAX_LEN: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct WindowContextId(Uuid);

impl WindowContextId {
    pub(crate) fn parse(raw: &str) -> Result<Self, WindowContextError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(WindowContextError::Empty);
        }
        if trimmed.len() > WINDOW_ID_MAX_LEN {
            return Err(WindowContextError::TooLong);
        }
        let parsed = Uuid::parse_str(trimmed).map_err(|_| WindowContextError::InvalidUuid)?;
        Ok(Self(parsed))
    }

    pub(crate) fn as_session_key(self) -> String {
        self.0.hyphenated().to_string()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum WindowContextError {
    InvalidUtf8,
    Empty,
    TooLong,
    InvalidUuid,
}

impl WindowContextError {
    pub(crate) const fn summary(self) -> &'static str {
        match self {
            Self::InvalidUtf8 => "window id must be valid UTF-8",
            Self::Empty => "window id must not be empty",
            Self::TooLong => "window id must be 128 bytes or shorter",
            Self::InvalidUuid => "window id must be a UUID",
        }
    }
}

pub(crate) fn resolve_window_context(
    headers: &HeaderMap,
) -> Result<Option<WindowContextId>, WindowContextError> {
    let Some(value) = headers.get(WINDOW_ID_HEADER) else {
        return Ok(None);
    };
    let Ok(value) = value.to_str() else {
        return Err(WindowContextError::InvalidUtf8);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    WindowContextId::parse(trimmed).map(Some)
}
