//! Domain error taxonomy and error classification for retry decisions.

use crate::entity::EntityRef;
use crate::ids::DispatchId;
use crate::policy::{PolicyDecisionKind, PolicyResourceRef};

/// Canonical domain errors returned by orchestration logic.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DomainError {
    // -- Guard violations --------------------------------------------------
    /// Another execute step is already pending or running for this dispatch.
    #[error("concurrent execute blocked for dispatch {dispatch_id}")]
    ConcurrentExecute { dispatch_id: DispatchId },

    /// Cannot execute after a teardown has been enqueued or completed.
    #[error("post-teardown execute blocked for dispatch {dispatch_id}")]
    PostTeardownExecute { dispatch_id: DispatchId },

    /// Cannot teardown while an execute step is still active.
    #[error("active execute blocks teardown for dispatch {dispatch_id}")]
    ActiveExecuteTeardown { dispatch_id: DispatchId },

    /// A teardown has already been enqueued or completed for this dispatch.
    #[error("duplicate teardown for dispatch {dispatch_id}")]
    DuplicateTeardown { dispatch_id: DispatchId },

    // -- Policy ------------------------------------------------------------
    /// A policy decision denied the requested action.
    #[error("policy denied ({kind}) for {resource}: {reason}")]
    PolicyDenied {
        kind: PolicyDecisionKind,
        resource: PolicyResourceRef,
        reason: String,
    },

    /// The budget limit has been exceeded.
    ///
    /// The `f64` fields here are intentionally not wrapped in
    /// [`crate::validated::FiniteF64`] — `DomainError` does **not**
    /// derive `Serialize` and never crosses the `SeaORM` JSON boundary.
    /// Errors are mapped to transport representations by downstream
    /// crates, so the "finite-only persisted floats" contract does not
    /// apply here.
    #[error("budget exceeded: limit={limit}, current={current}")]
    BudgetExceeded { limit: f64, current: f64 },

    /// A resource quota has been exhausted.
    #[error("quota exhausted for {resource}: limit={limit}")]
    QuotaExhausted { resource: String, limit: u64 },

    // -- Preconditions -----------------------------------------------------
    /// The requested entity was not found.
    #[error("{entity} not found")]
    NotFound { entity: EntityRef },

    /// The requested state transition is not valid.
    #[error("invalid transition {from} -> {to} for {entity}")]
    InvalidTransition {
        from: String,
        to: String,
        entity: EntityRef,
    },

    /// A general precondition was not satisfied.
    #[error("precondition failed: {reason}")]
    PreconditionFailed { reason: String },

    // -- Conflict ----------------------------------------------------------
    /// A conflicting operation is in progress.
    #[error("conflict: {reason}")]
    Conflict { reason: String },

    // -- Validation --------------------------------------------------------
    /// A domain value failed construction-time validation.
    #[error("invalid value for {field}: {reason}")]
    InvalidValue { field: String, reason: String },
}

impl std::fmt::Display for PolicyResourceRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dispatch { dispatch_id } => write!(f, "dispatch {dispatch_id}"),
            Self::Step {
                dispatch_id,
                step_id,
            } => write!(f, "step {step_id} (dispatch {dispatch_id})"),
            Self::Lease {
                dispatch_id,
                lease_id,
            } => write!(f, "lease {lease_id} (dispatch {dispatch_id})"),
            Self::Org { org_id } => write!(f, "org {org_id}"),
            Self::Team { org_id, team_id } => write!(f, "team {team_id} (org {org_id})"),
            Self::Project { project_id } => write!(f, "project {project_id}"),
            Self::Budget { org_id, envelope } => {
                write!(f, "budget:{envelope} (org {org_id})")
            }
            Self::Quota { org_id, resource } => {
                write!(f, "quota:{resource} (org {org_id})")
            }
        }
    }
}

/// Classification of an error for retry/escalation decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    /// Retryable — the operation may succeed on a subsequent attempt.
    Transient,
    /// Not retryable — the operation will never succeed without intervention.
    Fatal,
    /// Unknown — requires human or policy judgment.
    Ambiguous,
}

impl std::fmt::Display for ErrorClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transient => f.write_str("transient"),
            Self::Fatal => f.write_str("fatal"),
            Self::Ambiguous => f.write_str("ambiguous"),
        }
    }
}

/// Backoff durations (seconds) for transient retries.
pub const TRANSIENT_BACKOFF: [u64; 3] = [10, 30, 60];

/// Patterns in stdout/stderr that indicate transient failures.
const TRANSIENT_PATTERNS: &[&str] = &[
    "rate limit",
    "rate_limit",
    "429",
    "connection refused",
    "econnreset",
    "etimedout",
    "timeout",
    "503",
    "500",
    "temporarily unavailable",
];

/// Patterns in stdout/stderr that indicate fatal failures.
const FATAL_PATTERNS: &[&str] = &[
    "authentication_error",
    "401",
    "permission denied",
    "403",
    "command not found",
    "enoent",
];

/// Classify an error based on exit code, output, and signal.
///
/// Classification precedence:
/// 1. Signal `"error"` → Fatal
/// 2. Exit code 137 (OOM / SIGKILL) → Transient
/// 3. Pattern match on stdout/stderr
/// 4. Fallback → Ambiguous
#[must_use]
pub fn classify_error(
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
    signal: Option<&str>,
) -> ErrorClass {
    if signal == Some("error") {
        return ErrorClass::Fatal;
    }

    if exit_code == Some(137) {
        return ErrorClass::Transient;
    }

    let combined_lower = format!("{stdout}\n{stderr}").to_lowercase();

    for pattern in TRANSIENT_PATTERNS {
        if combined_lower.contains(pattern) {
            return ErrorClass::Transient;
        }
    }

    for pattern in FATAL_PATTERNS {
        if combined_lower.contains(pattern) {
            return ErrorClass::Fatal;
        }
    }

    ErrorClass::Ambiguous
}
