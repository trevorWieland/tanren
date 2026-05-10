//! Bounded capture types for subprocess output.
//!
//! [`BoundedCapture`] wraps stdout/stderr with explicit truncation
//! metadata so assertion errors never clone full output strings.
//! [`RedactedDiagnostic`] provides a safe view for error messages.

/// Maximum number of bytes captured from subprocess stdout or stderr.
/// Captures exceeding this bound are truncated and the truncation is
/// recorded in [`TruncationMeta`].
pub const CAPTURE_BOUND: usize = 16 * 1024;

/// Typed exit status for a subprocess invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandExitStatus {
    /// Process exited with the given code.
    Exited(i32),
    /// Process terminated by signal (code unavailable).
    Signal,
}

impl CommandExitStatus {
    /// Returns the exit code, if available.
    #[must_use]
    pub const fn code(self) -> Option<i32> {
        match self {
            Self::Exited(c) => Some(c),
            Self::Signal => None,
        }
    }

    /// Returns `true` when the process exited with code 0.
    #[must_use]
    pub const fn is_success(self) -> bool {
        matches!(self, Self::Exited(0))
    }
}

impl std::fmt::Display for CommandExitStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exited(code) => write!(f, "exited({code})"),
            Self::Signal => write!(f, "signal"),
        }
    }
}

/// Truncation metadata for a bounded capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TruncationMeta {
    /// Original byte length before truncation.
    pub original_len: usize,
    /// Whether the capture was truncated.
    pub was_truncated: bool,
}

impl TruncationMeta {
    /// Build truncation metadata from the original and bounded lengths.
    #[must_use]
    pub const fn new(original_len: usize, bound: usize) -> Self {
        Self {
            original_len,
            was_truncated: original_len > bound,
        }
    }
}

impl std::fmt::Display for TruncationMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.was_truncated {
            write!(f, "truncated {}/{} bytes", self.original_len, CAPTURE_BOUND)
        } else {
            write!(f, "intact ({} bytes)", self.original_len)
        }
    }
}

/// A bounded, UTF-8 string capture with explicit truncation metadata.
///
/// The inner string is at most [`CAPTURE_BOUND`] bytes. When the raw
/// capture exceeds that bound, the string is truncated and
/// [`TruncationMeta::was_truncated`] is set to `true`.
#[derive(Clone, PartialEq, Eq)]
pub struct BoundedCapture {
    text: String,
    truncation: TruncationMeta,
}

impl BoundedCapture {
    /// Create a bounded capture from raw bytes, truncating at
    /// [`CAPTURE_BOUND`].
    pub fn from_bytes(raw: &[u8]) -> Self {
        let text = String::from_utf8_lossy(raw).into_owned();
        Self::from_string(text)
    }

    /// Create a bounded capture from an owned string, truncating at
    /// [`CAPTURE_BOUND`].
    pub fn from_string(text: String) -> Self {
        let original_len = text.len();
        let truncation = TruncationMeta::new(original_len, CAPTURE_BOUND);
        let text = if truncation.was_truncated {
            // Find a char boundary near the bound to avoid panics.
            let mut end = CAPTURE_BOUND;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            text[..end].to_owned()
        } else {
            text
        };
        Self { text, truncation }
    }

    /// Access the captured text (possibly truncated).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Truncation metadata for this capture.
    #[must_use]
    pub const fn truncation(&self) -> &TruncationMeta {
        &self.truncation
    }

    /// Original byte length before truncation was applied.
    #[must_use]
    pub const fn original_len(&self) -> usize {
        self.truncation.original_len
    }
}

impl std::fmt::Debug for BoundedCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundedCapture")
            .field("text", &"<redacted>")
            .field("len", &self.text.len())
            .field("truncation", &self.truncation)
            .finish()
    }
}

/// A redacted diagnostic view suitable for inclusion in assertion error
/// messages. Carries only enough information to identify the failure
/// without cloning full stdout/stderr content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedDiagnostic {
    /// Typed exit status (absent when the command has not run).
    pub exit_status: Option<CommandExitStatus>,
    /// Truncation metadata for stdout.
    pub stdout_meta: TruncationMeta,
    /// Truncation metadata for stderr.
    pub stderr_meta: TruncationMeta,
}

impl std::fmt::Display for RedactedDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status_label = match &self.exit_status {
            Some(s) => s.to_string(),
            None => "unknown".to_owned(),
        };
        write!(
            f,
            "status={status_label}, stdout={stdout_meta}, stderr={stderr_meta}",
            stdout_meta = self.stdout_meta,
            stderr_meta = self.stderr_meta,
        )
    }
}
