//! Runtime trait contracts for harness and environment abstractions.
//!
//! Lane 1.1 scope in this crate focuses on the harness side:
//! - capability negotiation and preflight admissibility checks
//! - normalized execution and failure surfaces
//! - redaction-before-persistence contract
//! - reusable adapter conformance test helpers
//!
//! Provider-specific implementations live in `tanren-harness-*`.

mod adapter;
mod capability;
mod conformance;
mod execution;
mod failure;
mod redaction;

pub use adapter::{
    ExecutionSignal, HarnessAdapter, HarnessContractError, HarnessExecutionEvent, HarnessObserver,
    execute_with_contract,
};
pub use capability::{
    ApprovalMode, CapabilityAdmissibility, CompatibilityDenial, CompatibilityDenialKind,
    HarnessCapabilities, HarnessRequirements, OutputStreaming, OutputStreamingRequirement,
    PatchApplySupport, RequirementLevel, SandboxMode, SessionResumeSupport,
};
pub use conformance::{
    ConformanceEventRecorder, ConformanceResult, assert_capability_denial_is_preflight,
    assert_failure_classification, assert_redaction_before_persistence,
};
pub use execution::{
    HarnessExecutionRequest, HarnessExecutionResult, PersistableOutput, RawExecutionOutput,
};
pub use failure::{
    HarnessFailure, HarnessFailureClass, ProviderFailureContext, classify_provider_failure,
};
pub use redaction::{
    DefaultOutputRedactor, OutputRedactor, RedactionError, RedactionHints, RedactionPolicy,
    default_redaction_policy,
};
