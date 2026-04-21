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
    ContractCallToken, ExecutionSignal, HarnessAdapter, HarnessContractError, HarnessEventSource,
    HarnessExecutionEvent, HarnessExecutionEventKind, HarnessObserver, ProviderMetadataViolation,
    execute_with_contract,
};
pub use capability::{
    ApprovalMode, CapabilityAdmissibility, CompatibilityDenial, CompatibilityDenialKind,
    HarnessCapabilities, HarnessRequirements, OutputStreaming, OutputStreamingRequirement,
    PatchApplyRequirement, PatchApplySupport, RequirementLevel, SandboxMode,
    SessionResumeRequirement, SessionResumeSupport,
};
pub use conformance::{
    ConformanceEventRecorder, ConformanceResult, RedactionConformanceExpectations,
    assert_capability_denial_is_preflight, assert_failure_classification,
    assert_redaction_before_persistence,
};
pub use execution::{
    HarnessExecutionRequest, HarnessExecutionResult, PersistableOutput, RawExecutionOutput,
    RedactionSecret, SecretName,
};
pub use failure::{
    HarnessFailure, HarnessFailureClass, ProviderFailure, ProviderFailureCode,
    ProviderFailureContext, ProviderIdentifier, ProviderIdentifierError, TerminalFailureCodeError,
    classify_provider_failure, classify_provider_failure_for_audit,
};
pub use redaction::{
    DefaultOutputRedactor, OutputRedactor, RedactionAudit, RedactionError, RedactionHints,
    RedactionOutcome, RedactionPolicy, default_redaction_policy,
    default_redaction_policy_dataset_version,
};
