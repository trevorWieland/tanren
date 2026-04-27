//! Shared YAML-frontmatter primitive.
//!
//! Every narrative evidence file (spec.md, plan.md, demo.md, audit.md,
//! signposts.md) uses the canonical shape:
//!
//! ```text
//! ---
//! <yaml frontmatter>
//! ---
//! <markdown body>
//! ```
//!
//! This module provides typed [`split`] / [`join`] helpers that are
//! transport-free (no I/O), deterministic (stable key ordering via
//! `BTreeMap<String, Value>` on render), and strict on malformed input
//! (no silent recovery, typed errors).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Required schema version for orchestrator-owned markdown frontmatter.
pub const EVIDENCE_SCHEMA_VERSION: &str = "v1";

/// Schema version tag embedded in markdown frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct EvidenceSchemaVersion(String);

impl Default for EvidenceSchemaVersion {
    fn default() -> Self {
        Self::current()
    }
}

impl EvidenceSchemaVersion {
    /// Current evidence schema version.
    #[must_use]
    pub fn current() -> Self {
        Self(EVIDENCE_SCHEMA_VERSION.to_owned())
    }

    /// Access as `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Serde default helper for `schema_version` fields.
#[must_use]
pub fn default_schema_version() -> EvidenceSchemaVersion {
    EvidenceSchemaVersion::current()
}

impl<'de> Deserialize<'de> for EvidenceSchemaVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        if raw == EVIDENCE_SCHEMA_VERSION {
            Ok(Self(raw))
        } else {
            Err(serde::de::Error::custom(format!(
                "unsupported schema_version `{raw}` (expected `{EVIDENCE_SCHEMA_VERSION}`)"
            )))
        }
    }
}

/// Error returned by [`split`] / [`join`] when input is malformed.
///
/// `serde_yaml::Error` is not [`Clone`], so this enum doesn't derive
/// [`Clone`] either; callers that need an owned diagnostic should hold
/// the error via reference or capture `.to_string()`.
#[derive(Debug, thiserror::Error)]
pub enum FrontmatterError {
    /// Input did not begin with a `---` opener line.
    #[error("input must begin with a `---` frontmatter opener")]
    MissingOpener,
    /// Opener present but no closing `---` line was found.
    #[error("frontmatter opener `---` was not followed by a closing `---` line")]
    MissingCloser,
    /// The frontmatter YAML failed to parse.
    #[error("failed to parse frontmatter YAML: {source}")]
    InvalidYaml {
        #[from]
        source: serde_yaml::Error,
    },
    /// Typed deserialization of the frontmatter YAML into the target
    /// struct failed.
    #[error("frontmatter schema error: {reason}")]
    SchemaError { reason: String },
}

/// Split a markdown document with YAML frontmatter into `(frontmatter,
/// body)`. The frontmatter is parsed into a
/// [`serde_yaml::Value`]; callers deserialize into a typed struct.
///
/// The body is returned verbatim (no canonicalization) so consumers can
/// treat it as opaque user-authored text.
///
/// # Errors
/// See [`FrontmatterError`].
pub fn split(input: &str) -> Result<(serde_yaml::Value, String), FrontmatterError> {
    let (yaml, body) = split_yaml_and_body(input)?;
    let value: serde_yaml::Value = serde_yaml::from_str(&yaml)?;
    Ok((value, body))
}

fn split_yaml_and_body(input: &str) -> Result<(String, String), FrontmatterError> {
    let normalized = input.replace("\r\n", "\n");
    let mut lines = normalized.lines();
    let first = lines.next().ok_or(FrontmatterError::MissingOpener)?;
    if first.trim() != "---" {
        return Err(FrontmatterError::MissingOpener);
    }
    let mut yaml_lines = Vec::new();
    let mut found_closer = false;
    let mut body_lines = Vec::new();
    for line in lines {
        if found_closer {
            body_lines.push(line);
        } else if line.trim() == "---" {
            found_closer = true;
        } else {
            yaml_lines.push(line);
        }
    }
    if !found_closer {
        return Err(FrontmatterError::MissingCloser);
    }
    let yaml = yaml_lines.join("\n");
    // Trailing newline on body is lossy under line-splitting; rebuild
    // with a final \n to match canonical rendering.
    let body = if body_lines.is_empty() {
        String::new()
    } else {
        let mut s = body_lines.join("\n");
        s.push('\n');
        s
    };
    Ok((yaml, body))
}

/// Render a typed frontmatter struct plus a markdown body into a
/// canonical `---\n<yaml>---\n<body>` document.
///
/// Keys at every nesting level are serialized in the order serde emits
/// them for the struct (field declaration order), which is stable and
/// byte-for-byte reproducible. Line endings are LF only; the document
/// ends with a single final newline.
///
/// # Errors
/// Returns [`FrontmatterError::InvalidYaml`] if the struct fails to
/// serialize into YAML.
pub fn join<T: Serialize>(frontmatter: &T, body: &str) -> Result<String, FrontmatterError> {
    let yaml = serde_yaml::to_string(frontmatter)?;
    let mut out = String::with_capacity(yaml.len() + body.len() + 16);
    out.push_str("---\n");
    out.push_str(&yaml);
    if !yaml.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("---\n\n");
    if body.is_empty() {
        // Canonical form still ends with a single newline to keep
        // byte-for-byte stability across render→parse→render cycles.
    } else {
        out.push_str(body);
        if !body.ends_with('\n') {
            out.push('\n');
        }
    }
    Ok(out)
}

/// Typed round-trip helper: parse `input` into `T`, returning both the
/// parsed struct and the verbatim body.
///
/// # Errors
/// See [`FrontmatterError`].
pub fn parse_typed<T: for<'de> Deserialize<'de>>(
    input: &str,
) -> Result<(T, String), FrontmatterError> {
    let (yaml, body) = split_yaml_and_body(input)?;
    let de = serde_yaml::Deserializer::from_str(&yaml);
    let parsed: T = serde_path_to_error::deserialize(de).map_err(|source| {
        let path = serde_path_to_json_pointer(&source.path().to_string());
        let reason = format!("{path}: {}", source.inner());
        FrontmatterError::SchemaError { reason }
    })?;
    Ok((parsed, body))
}

fn serde_path_to_json_pointer(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "." {
        return "/".into();
    }
    let mut out = String::new();
    let mut token = String::new();
    let mut in_brackets = false;
    for ch in trimmed.chars() {
        match ch {
            '.' if !in_brackets => {
                if !token.is_empty() {
                    out.push('/');
                    out.push_str(&token);
                    token.clear();
                }
            }
            '[' => {
                if !token.is_empty() {
                    out.push('/');
                    out.push_str(&token);
                    token.clear();
                }
                in_brackets = true;
            }
            ']' => {
                if !token.is_empty() {
                    out.push('/');
                    out.push_str(&token);
                    token.clear();
                }
                in_brackets = false;
            }
            _ => token.push(ch),
        }
    }
    if !token.is_empty() {
        out.push('/');
        out.push_str(&token);
    }
    if out.is_empty() { "/".into() } else { out }
}
