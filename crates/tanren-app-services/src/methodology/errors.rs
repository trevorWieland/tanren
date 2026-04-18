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

    #[error("internal error: {0}")]
    Internal(String),
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
            MethodologyError::Internal(msg) => Self::Internal {
                reason: msg.clone(),
            },
        }
    }
}

/// Convenient result alias.
pub type MethodologyResult<T> = Result<T, MethodologyError>;
