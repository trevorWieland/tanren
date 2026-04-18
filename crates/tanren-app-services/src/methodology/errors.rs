//! Methodology error umbrella.
//!
//! [`MethodologyError`] is the internal, source-of-truth error type;
//! [`ToolError`] is the wire-facing, typed-reason representation
//! returned to agent tool calls (via MCP + CLI). The two forms are
//! orthogonal: `MethodologyError` carries rich context for logs +
//! tracing; `ToolError` carries the actionable remediation shape per
//! `docs/architecture/agent-tool-surface.md` §5.

use serde::{Deserialize, Serialize};
use tanren_domain::methodology::capability::ToolCapability;

/// Source-of-truth methodology error.
#[derive(Debug, thiserror::Error)]
pub enum MethodologyError {
    #[error("domain error: {0}")]
    Domain(#[from] tanren_domain::DomainError),

    #[error("store error: {0}")]
    Store(#[from] tanren_store::StoreError),

    #[error("projection error: {0}")]
    Projection(#[from] tanren_store::methodology::projections::MethodologyEventFetchError),

    #[error("validation failed: {0}")]
    Validation(String),

    /// Structured validation failure with a JSON-pointer field path.
    /// Prefer this over [`Self::Validation`] when you know the offending
    /// field, so the ToolError surface carries real precision per
    /// `agent-tool-surface.md` §5.
    #[error("validation failed at {field_path}: expected {expected}, got {actual}")]
    FieldValidation {
        field_path: String,
        expected: String,
        actual: String,
        remediation: String,
    },

    #[error("rubric invariant violated for {pillar} at score {score}: {reason}")]
    RubricInvariantViolated {
        pillar: String,
        score: u8,
        reason: String,
    },

    #[error("capability denied: {capability} not allowed in phase `{phase}`")]
    CapabilityDenied {
        capability: ToolCapability,
        phase: String,
    },

    #[error("illegal task transition: task {task_id} {from} → {attempted}")]
    IllegalTaskTransition {
        task_id: tanren_domain::TaskId,
        from: String,
        attempted: String,
    },

    #[error("not found: {resource} `{key}`")]
    NotFound { resource: String, key: String },

    #[error("conflict: {resource}: {reason}")]
    Conflict { resource: String, reason: String },

    #[error("evidence schema error in {file}: {reason}")]
    EvidenceSchema { file: String, reason: String },

    #[error("I/O error on {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A JSONL replay/ingest line could not be parsed. Carries the
    /// line number and a short snippet of the raw payload so the
    /// caller can fix the input without re-scanning the whole file.
    /// Kept structured (rather than collapsed to `Internal`) so the
    /// CLI boundary can surface `{code, line, raw}` per the audit
    /// remediation plan.
    #[error("malformed replay line {line} at {path}: {reason}")]
    ReplayMalformedLine {
        path: std::path::PathBuf,
        line: usize,
        reason: String,
        raw: String,
    },

    /// A JSONL replay line parsed but its envelope failed to decode.
    /// Separate from `ReplayMalformedLine` so tests can assert the
    /// specific failure class.
    #[error("replay envelope decode error at {path}:{line}: {reason}")]
    ReplayEnvelopeDecode {
        path: std::path::PathBuf,
        line: usize,
        reason: String,
    },

    #[error("internal error: {0}")]
    Internal(String),
}

impl From<tanren_store::methodology::replay::ReplayError> for MethodologyError {
    fn from(err: tanren_store::methodology::replay::ReplayError) -> Self {
        use tanren_store::methodology::replay::ReplayError as R;
        match err {
            R::Io { path, source } => Self::Io { path, source },
            R::MalformedLine {
                path,
                line,
                reason,
                raw,
            } => Self::ReplayMalformedLine {
                path,
                line,
                reason,
                raw,
            },
            R::EnvelopeDecode { path, line, reason } => {
                Self::ReplayEnvelopeDecode { path, line, reason }
            }
            R::Store { source } => Self::Store(source),
        }
    }
}

/// Agent-facing typed error shape.
///
/// Serde-serialized into the MCP tool-error result and the CLI's
/// structured error output. Field names match `agent-tool-surface.md`
/// §5 verbatim so the shape is cross-interface stable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolError {
    ValidationFailed {
        /// JSON-pointer path to the offending field.
        field_path: String,
        expected: String,
        actual: String,
        remediation: String,
    },
    CapabilityDenied {
        capability: String,
        phase: String,
    },
    IllegalTaskTransition {
        task_id: tanren_domain::TaskId,
        from: String,
        attempted: String,
    },
    RubricInvariantViolated {
        pillar: String,
        score: u8,
        reason: String,
    },
    Conflict {
        resource: String,
        reason: String,
    },
    NotFound {
        resource: String,
        key: String,
    },
    Internal {
        reason: String,
    },
}

impl From<&MethodologyError> for ToolError {
    fn from(err: &MethodologyError) -> Self {
        match err {
            MethodologyError::Validation(msg) => validation_from_message(msg),
            MethodologyError::FieldValidation {
                field_path,
                expected,
                actual,
                remediation,
            } => Self::ValidationFailed {
                field_path: field_path.clone(),
                expected: expected.clone(),
                actual: actual.clone(),
                remediation: remediation.clone(),
            },
            MethodologyError::RubricInvariantViolated {
                pillar,
                score,
                reason,
            } => Self::RubricInvariantViolated {
                pillar: pillar.clone(),
                score: *score,
                reason: reason.clone(),
            },
            MethodologyError::CapabilityDenied { capability, phase } => Self::CapabilityDenied {
                capability: capability.tag().into(),
                phase: phase.clone(),
            },
            MethodologyError::IllegalTaskTransition {
                task_id,
                from,
                attempted,
            } => Self::IllegalTaskTransition {
                task_id: *task_id,
                from: from.clone(),
                attempted: attempted.clone(),
            },
            MethodologyError::NotFound { resource, key } => Self::NotFound {
                resource: resource.clone(),
                key: key.clone(),
            },
            MethodologyError::Conflict { resource, reason } => Self::Conflict {
                resource: resource.clone(),
                reason: reason.clone(),
            },
            MethodologyError::Domain(d) => Self::Internal {
                reason: d.to_string(),
            },
            MethodologyError::Store(s) => Self::Internal {
                reason: s.to_string(),
            },
            MethodologyError::Projection(p) => Self::Internal {
                reason: p.to_string(),
            },
            MethodologyError::EvidenceSchema { file, reason } => Self::ValidationFailed {
                field_path: format!("/{file}"),
                expected: "valid evidence frontmatter".into(),
                actual: "malformed".into(),
                remediation: reason.clone(),
            },
            MethodologyError::Io { path, source } => Self::Internal {
                reason: format!("I/O on {}: {source}", path.display()),
            },
            MethodologyError::ReplayMalformedLine {
                path,
                line,
                reason,
                raw,
            } => Self::ValidationFailed {
                field_path: format!("/{}:{line}", path.display()),
                expected: "valid envelope JSON".into(),
                actual: format!("raw={raw}"),
                remediation: reason.clone(),
            },
            MethodologyError::ReplayEnvelopeDecode { path, line, reason } => {
                Self::ValidationFailed {
                    field_path: format!("/{}:{line}", path.display()),
                    expected: "decodable event envelope".into(),
                    actual: "decode failed".into(),
                    remediation: reason.clone(),
                }
            }
            MethodologyError::Internal(msg) => Self::Internal {
                reason: msg.clone(),
            },
        }
    }
}

fn validation_from_message(msg: &str) -> ToolError {
    // Fall back to a best-effort inference when the caller didn't
    // emit FieldValidation. We parse `msg` for a leading
    // `"<field>: "` marker so stock error strings like
    // `"title: value cannot be empty"` still surface the field.
    // This path is intentionally imprecise; service methods now
    // prefer `FieldValidation`.
    let (field_path, remediation) = match msg.split_once(": ") {
        Some((lead, rest))
            if lead
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '/') =>
        {
            (format!("/{lead}"), rest.to_owned())
        }
        _ => ("/".to_owned(), msg.to_owned()),
    };
    ToolError::ValidationFailed {
        field_path,
        expected: "valid input per schema".into(),
        actual: "rejected".into(),
        remediation,
    }
}

/// Convenient result alias.
pub type MethodologyResult<T> = Result<T, MethodologyError>;

/// Helper: validate a required non-empty string field, emitting a
/// typed [`MethodologyError::FieldValidation`] with an actionable
/// JSON-pointer `field_path` on failure.
///
/// # Errors
/// Returns [`MethodologyError::FieldValidation`] when `value` is empty
/// or whitespace-only.
pub fn require_non_empty(
    field_path: &str,
    value: &str,
    max_len: Option<usize>,
) -> MethodologyResult<tanren_domain::NonEmptyString> {
    match tanren_domain::NonEmptyString::try_new(value.to_owned()) {
        Ok(s) => {
            if let Some(max) = max_len
                && s.as_str().chars().count() > max
            {
                return Err(MethodologyError::FieldValidation {
                    field_path: field_path.into(),
                    expected: format!("non-empty string ≤ {max} chars"),
                    actual: format!("{} chars", s.as_str().chars().count()),
                    remediation: format!(
                        "shorten the value at `{field_path}` to {max} or fewer characters"
                    ),
                });
            }
            Ok(s)
        }
        Err(_) => Err(MethodologyError::FieldValidation {
            field_path: field_path.into(),
            expected: "non-empty string".into(),
            actual: format!("{value:?}"),
            remediation: format!("supply a non-empty value for `{field_path}`"),
        }),
    }
}
