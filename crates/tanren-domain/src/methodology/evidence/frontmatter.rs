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
    out.push_str("---\n");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct Sample {
        kind: String,
        n: u32,
    }

    #[test]
    fn roundtrip_basic() {
        let s = Sample {
            kind: "demo".into(),
            n: 7,
        };
        let doc = join(&s, "body here\n").expect("join");
        assert!(doc.starts_with("---\n"));
        assert!(doc.contains("kind: demo"));
        assert!(doc.ends_with("body here\n"));
        let (back, body): (Sample, String) = parse_typed(&doc).expect("parse");
        assert_eq!(back, s);
        assert_eq!(body, "body here\n");
    }

    #[test]
    fn empty_body_roundtrip_stable() {
        let s = Sample {
            kind: "empty".into(),
            n: 0,
        };
        let doc = join(&s, "").expect("join");
        let (back, body): (Sample, String) = parse_typed(&doc).expect("parse");
        assert_eq!(back, s);
        assert_eq!(body, "");
        // second round: produce bytes identical to the first render
        let doc2 = join(&back, &body).expect("join2");
        assert_eq!(doc, doc2);
    }

    #[test]
    fn missing_opener_fails() {
        let err = split("no frontmatter here").expect_err("must fail");
        assert!(matches!(err, FrontmatterError::MissingOpener));
    }

    #[test]
    fn missing_closer_fails() {
        let err = split("---\nkind: x\nno closer").expect_err("must fail");
        assert!(matches!(err, FrontmatterError::MissingCloser));
    }

    #[test]
    fn crlf_is_normalized() {
        let input = "---\r\nkind: demo\r\nn: 1\r\n---\r\nbody\r\n";
        let (value, body) = split(input).expect("split");
        assert_eq!(value["kind"], "demo");
        assert_eq!(body, "body\n");
    }

    #[test]
    fn schema_error_is_typed() {
        let input = "---\nkind: demo\nn: not-a-number\n---\nbody\n";
        let err = parse_typed::<Sample>(input).expect_err("must fail");
        assert!(matches!(err, FrontmatterError::SchemaError { .. }));
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct Nested {
        label: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct Root {
        nested: Nested,
    }

    #[test]
    fn schema_error_reports_actionable_field_path() {
        let input = "---\nnested:\n  label:\n    bad: map\n---\nbody\n";
        let err = parse_typed::<Root>(input).expect_err("must fail");
        let msg = err.to_string();
        assert!(
            msg.contains("/nested/label"),
            "schema errors should include field path, got: {msg}"
        );
        assert!(
            msg.contains("expected a string"),
            "schema errors should preserve type mismatch reason, got: {msg}"
        );
    }

    #[test]
    fn extra_trailing_newlines_preserved_on_body() {
        let input = "---\nkind: demo\nn: 1\n---\nline1\nline2\n";
        let (_, body) = split(input).expect("split");
        assert_eq!(body, "line1\nline2\n");
    }
}
