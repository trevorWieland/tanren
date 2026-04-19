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
use tanren_domain::methodology::validation::ValidationIssue;

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
            R::SpecIdMismatch {
                path,
                line,
                line_spec_id,
                payload_spec_id,
            } => replay_envelope_decode(
                path,
                line,
                format!("spec_id mismatch: line={line_spec_id}, payload={payload_spec_id}",),
            ),
            R::MissingPayloadSpecId { path, line } => {
                replay_envelope_decode(path, line, "payload missing spec_id")
            }
            R::ToolMismatch {
                path,
                line,
                expected,
                actual,
            } => replay_envelope_decode(
                path,
                line,
                format!("tool mismatch: expected `{expected}`, got `{actual}`"),
            ),
            R::OriginKindMismatch {
                path,
                line,
                expected,
                actual,
            } => replay_envelope_decode(
                path,
                line,
                format!("origin_kind mismatch: expected `{expected}`, got `{actual}`"),
            ),
            R::MissingOriginKind { path, line } => {
                replay_envelope_decode(path, line, "missing required origin_kind")
            }
            R::MissingCausedByToolCall { path, line, origin } => replay_envelope_decode(
                path,
                line,
                format!("missing caused_by_tool_call_id for origin `{origin}`"),
            ),
            R::FieldValidation { details } => Self::FieldValidation {
                field_path: details.field_path,
                expected: details.expected,
                actual: details.actual,
                remediation: details.remediation,
            },
            R::InvalidTaskTransition {
                task_id,
                from,
                attempted,
                ..
            } => Self::IllegalTaskTransition {
                task_id,
                from,
                attempted,
            },
            R::MissingTaskCreate {
                path,
                line,
                task_id,
            } => replay_envelope_decode(
                path,
                line,
                format!("missing TaskCreated for task {task_id}"),
            ),
            R::DuplicateTaskCreate {
                path,
                line,
                task_id,
            } => replay_envelope_decode(
                path,
                line,
                format!("duplicate TaskCreated for task {task_id}"),
            ),
            R::TaskCompletedMissingGuards {
                path,
                line,
                task_id,
            } => replay_envelope_decode(
                path,
                line,
                format!("TaskCompleted before required guards for task {task_id}"),
            ),
            R::Store { source } => Self::Store(source),
        }
    }
}

fn replay_envelope_decode(
    path: std::path::PathBuf,
    line: usize,
    reason: impl Into<String>,
) -> MethodologyError {
    MethodologyError::ReplayEnvelopeDecode {
        path,
        line,
        reason: reason.into(),
    }
}

impl From<ValidationIssue> for MethodologyError {
    fn from(issue: ValidationIssue) -> Self {
        Self::FieldValidation {
            field_path: issue.field_path,
            expected: issue.expected,
            actual: issue.actual,
            remediation: issue.remediation,
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
            MethodologyError::Validation(msg) => Self::ValidationFailed {
                field_path: "/".into(),
                expected: "valid input per schema".into(),
                actual: "rejected".into(),
                remediation: msg.clone(),
            },
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

impl From<ToolError> for MethodologyError {
    fn from(value: ToolError) -> Self {
        match value {
            ToolError::ValidationFailed {
                field_path,
                expected,
                actual,
                remediation,
            } => Self::FieldValidation {
                field_path,
                expected,
                actual,
                remediation,
            },
            ToolError::CapabilityDenied { capability, phase } => {
                if let Some(parsed) = parse_tool_capability(&capability) {
                    Self::CapabilityDenied {
                        capability: parsed,
                        phase,
                    }
                } else {
                    Self::Internal(format!(
                        "unknown capability tag in replayed idempotency error: {capability}"
                    ))
                }
            }
            ToolError::IllegalTaskTransition {
                task_id,
                from,
                attempted,
            } => Self::IllegalTaskTransition {
                task_id,
                from,
                attempted,
            },
            ToolError::RubricInvariantViolated {
                pillar,
                score,
                reason,
            } => Self::RubricInvariantViolated {
                pillar,
                score,
                reason,
            },
            ToolError::Conflict { resource, reason } => Self::Conflict { resource, reason },
            ToolError::NotFound { resource, key } => Self::NotFound { resource, key },
            ToolError::Internal { reason } => Self::Internal(reason),
        }
    }
}

fn parse_tool_capability(tag: &str) -> Option<ToolCapability> {
    Some(match tag {
        "task.create" => ToolCapability::TaskCreate,
        "task.start" => ToolCapability::TaskStart,
        "task.complete" => ToolCapability::TaskComplete,
        "task.revise" => ToolCapability::TaskRevise,
        "task.abandon" => ToolCapability::TaskAbandon,
        "task.read" => ToolCapability::TaskRead,
        "finding.add" => ToolCapability::FindingAdd,
        "rubric.record" => ToolCapability::RubricRecord,
        "compliance.record" => ToolCapability::ComplianceRecord,
        "spec.frontmatter" => ToolCapability::SpecFrontmatter,
        "demo.frontmatter" => ToolCapability::DemoFrontmatter,
        "demo.results" => ToolCapability::DemoResults,
        "signpost.add" => ToolCapability::SignpostAdd,
        "signpost.update" => ToolCapability::SignpostUpdate,
        "phase.outcome" => ToolCapability::PhaseOutcome,
        "phase.escalate" => ToolCapability::PhaseEscalate,
        "issue.create" => ToolCapability::IssueCreate,
        "standard.read" => ToolCapability::StandardRead,
        "adherence.record" => ToolCapability::AdherenceRecord,
        "feedback.reply" => ToolCapability::FeedbackReply,
        _ => return None,
    })
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
