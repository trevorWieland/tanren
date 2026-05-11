//! Typed CLI output encoder/decoder for install and drift commands.
//!
//! Provides a single typed encoding path so that `tanren-cli` never writes
//! raw string literals for labels, and paths/values containing whitespace,
//! `=`, `]`, control chars, or newlines round-trip losslessly through
//! percent-encoding. The testkit parsers consume the same encoded form.

use std::fmt;

// ---------------------------------------------------------------------------
// Percent-encoding helpers
// ---------------------------------------------------------------------------

/// Characters that trigger percent-encoding in output field values.
///
/// The set covers whitespace, the field delimiter `=`, the bracket delimiter
/// `]`, and all C0 control characters (0x00–0x1F) plus DEL (0x7F).
const ENCODE_SET: fn(u8) -> bool =
    |b: u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'=' | b']' | b'%') || b.is_ascii_control();

/// Percent-encode a value for safe inclusion in key=value CLI output.
///
/// Every byte in `ENCODE_SET` is replaced with `%XX` (uppercase hex).
/// The encoder is deterministic and injective — two distinct inputs always
/// produce distinct encoded outputs.
#[must_use]
pub fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if ENCODE_SET(byte) {
            out.push('%');
            out.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
            out.push(char::from(b"0123456789ABCDEF"[(byte & 0x0F) as usize]));
        } else {
            out.push(char::from(byte));
        }
    }
    out
}

/// Percent-decode a value that was encoded with [`percent_encode`].
///
/// Returns `None` if the input contains an incomplete `%XX` sequence or
/// non-hex-digit characters after `%`.
#[must_use]
pub fn percent_decode(encoded: &str) -> Option<String> {
    let mut out = String::with_capacity(encoded.len());
    let mut chars = encoded.bytes();
    while let Some(b) = chars.next() {
        if b != b'%' {
            out.push(char::from(b));
            continue;
        }
        let hi = chars.next()?;
        let lo = chars.next()?;
        let hi_val = hex_nibble(hi)?;
        let lo_val = hex_nibble(lo)?;
        out.push(char::from((hi_val << 4) | lo_val));
    }
    Some(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Typed output labels
// ---------------------------------------------------------------------------

/// Top-level record discriminant — the first whitespace-delimited token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKind {
    /// Install/drift summary line.
    Summary,
    /// Per-path detail line.
    Detail,
    /// Install paths line.
    Paths,
}

impl RecordKind {
    /// Wire representation of this record kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Detail => "detail",
            Self::Paths => "paths",
        }
    }
}

impl fmt::Display for RecordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Typed key for a key=value output field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKey {
    /// `status` — overall command or per-path status.
    Status,
    /// `command` — command name (`install` or `drift`).
    Command,
    /// `repo` — repository argument (redacted if absolute).
    Repo,
    /// `profile` — standards profile name.
    Profile,
    /// `integrations` — comma-separated integration identifiers.
    Integrations,
    /// `path` — repo-relative path in a detail record.
    Path,
    /// `created` — count or path list for created assets.
    Created,
    /// `updated` — count or path list for updated assets.
    Updated,
    /// `removed` — count or path list for removed assets.
    Removed,
    /// `restored` — count or path list for restored assets.
    Restored,
    /// `preserved` — count or path list for preserved assets.
    Preserved,
    /// `clean` — count of clean paths.
    Clean,
    /// `changed_generated` — count of changed generated assets.
    ChangedGenerated,
    /// `missing_generated` — count of missing generated assets.
    MissingGenerated,
    /// `missing_preserved` — count of missing preserved standards.
    MissingPreserved,
    /// `accepted_preserved` — count of accepted preserved edits.
    AcceptedPreserved,
    /// `drift` — total drift count.
    Drift,
}

impl OutputKey {
    /// Wire representation of this key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Command => "command",
            Self::Repo => "repo",
            Self::Profile => "profile",
            Self::Integrations => "integrations",
            Self::Path => "path",
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Removed => "removed",
            Self::Restored => "restored",
            Self::Preserved => "preserved",
            Self::Clean => "clean",
            Self::ChangedGenerated => "changed_generated",
            Self::MissingGenerated => "missing_generated",
            Self::MissingPreserved => "missing_preserved",
            Self::AcceptedPreserved => "accepted_preserved",
            Self::Drift => "drift",
        }
    }
}

impl fmt::Display for OutputKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Install command overall status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStatus {
    /// Install succeeded.
    Ok,
}

impl InstallStatus {
    /// Wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        "ok"
    }
}

impl fmt::Display for InstallStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Drift command overall status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftStatus {
    /// No drift detected.
    Ok,
    /// One or more paths drifted.
    Drift,
}

impl DriftStatus {
    /// Wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Drift => "drift",
        }
    }

    /// Parse from wire representation.
    ///
    /// Returns `None` for unknown status strings.
    #[must_use]
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "ok" => Some(Self::Ok),
            "drift" => Some(Self::Drift),
            _ => None,
        }
    }
}

impl fmt::Display for DriftStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Per-path drift detail status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftDetailStatus {
    /// Repository file matches the current install projection.
    Clean,
    /// Tanren-owned generated file exists but content differs from projection.
    ChangedGenerated,
    /// Tanren-owned generated file is missing from the repository.
    MissingGenerated,
    /// Preserved standards profile file is missing from the repository.
    MissingPreserved,
    /// Preserved standards profile file was edited and accepted as user-owned.
    AcceptedPreserved,
}

impl DriftDetailStatus {
    /// Wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::ChangedGenerated => "changed_generated",
            Self::MissingGenerated => "missing_generated",
            Self::MissingPreserved => "missing_preserved",
            Self::AcceptedPreserved => "accepted_preserved",
        }
    }

    /// Parse from wire representation.
    ///
    /// Returns `None` for unknown status strings.
    #[must_use]
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "clean" => Some(Self::Clean),
            "changed_generated" => Some(Self::ChangedGenerated),
            "missing_generated" => Some(Self::MissingGenerated),
            "missing_preserved" => Some(Self::MissingPreserved),
            "accepted_preserved" => Some(Self::AcceptedPreserved),
            _ => None,
        }
    }
}

impl fmt::Display for DriftDetailStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Command name for output encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandName {
    /// `install` command.
    Install,
    /// `drift` command.
    Drift,
}

impl CommandName {
    /// Wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Drift => "drift",
        }
    }
}

impl fmt::Display for CommandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Field encoder helpers
// ---------------------------------------------------------------------------

/// Write a single `key=value` pair with the value percent-encoded.
pub fn write_field(out: &mut impl fmt::Write, key: OutputKey, value: &str) -> fmt::Result {
    write!(out, " {}={}", key, percent_encode(value))
}

/// Write a single `key=<usize>` pair (no encoding needed for integers).
pub fn write_count_field(out: &mut impl fmt::Write, key: OutputKey, count: usize) -> fmt::Result {
    write!(out, " {key}={count}")
}

/// Write a `key=<enum>` pair using the enum's [`fmt::Display`] impl.
pub fn write_typed_field<T: fmt::Display>(
    out: &mut impl fmt::Write,
    key: OutputKey,
    value: T,
) -> fmt::Result {
    write!(out, " {key}={value}")
}

/// Write a `key=[path1,path2,...]` bracket list with each path percent-encoded.
pub fn write_bracket_list(
    out: &mut impl fmt::Write,
    key: OutputKey,
    paths: &[&str],
) -> fmt::Result {
    write!(out, " {key}=[")?;
    for (i, path) in paths.iter().enumerate() {
        if i > 0 {
            out.write_str(",")?;
        }
        out.write_str(&percent_encode(path))?;
    }
    out.write_str("]")
}

// ---------------------------------------------------------------------------
// Field decoder helpers (used by testkit)
// ---------------------------------------------------------------------------

/// Parse `key=value` tokens from a line, percent-decoding all values.
///
/// This replaces the raw `split_whitespace` / `split_once('=')` approach
/// that would corrupt on whitespace or `=` inside values.
pub fn parse_kv_fields_percent_decoded(line: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for token in line.split_whitespace() {
        if let Some((key, value)) = token.split_once('=') {
            let decoded = percent_decode(value).unwrap_or_else(|| value.to_owned());
            map.insert(key.to_owned(), decoded);
        }
    }
    map
}

/// Parse a bracket-enclosed path list from a line, percent-decoding each entry.
///
/// Looks for the pattern `key=[p1,p2,...]` in `line`, where each path may
/// be percent-encoded. Returns an empty vec if the pattern is not found.
pub fn parse_bracket_list_percent_decoded(line: &str, key: &str) -> Vec<String> {
    let marker = format!("{key}=[");
    let Some(start) = line.find(&marker) else {
        return Vec::new();
    };
    let content_start = start + marker.len();
    let remaining = &line[content_start..];
    let end = remaining.find(']').unwrap_or(remaining.len());
    if end == 0 {
        return Vec::new();
    }
    remaining[..end]
        .split(',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| percent_decode(s.trim()))
        .collect()
}
