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
    ContractCallToken, ExecutionSignal, HarnessAdapter, HarnessAdapterRegistry,
    HarnessAdapterRegistryError, HarnessContractError, HarnessEventSource, HarnessExecutionEvent,
    HarnessExecutionEventKind, HarnessObserver, ProviderMetadataViolation, execute_with_contract,
};
pub use capability::{
    ApprovalMode, ApprovalModeBounds, CapabilityAdmissibility, CompatibilityDenial,
    CompatibilityDenialKind, HarnessCapabilities, HarnessRequirements, HarnessRequirementsBuilder,
    OutputStreaming, OutputStreamingRequirement, PatchApplyRequirement, PatchApplySupport,
    RequirementBoundsError, RequirementLevel, SandboxMode, SandboxModeBounds,
    SessionResumeRequirement, SessionResumeSupport,
};
pub use conformance::{
    ConformanceEventRecorder, ConformanceResult, RedactionConformanceExpectations,
    assert_capability_denial_is_preflight, assert_failure_classification,
    assert_failure_path_leak_detected, assert_provider_metadata_fail_closed,
    assert_redaction_before_persistence, assert_terminal_typed_code_mapping,
};
pub use execution::{
    HarnessExecutionRequest, HarnessExecutionResult, PersistableOutput, RawExecutionOutput,
    RedactionSecret, SecretName,
};
pub use failure::{
    AuditProviderFailureCode, AuditProviderFailureContext, HarnessFailure, HarnessFailureClass,
    ProviderFailure, ProviderFailureCode, ProviderFailureContext, ProviderIdentifier,
    ProviderIdentifierError, ProviderRunId, ProviderRunIdError, classify_provider_failure,
    classify_provider_failure_for_audit,
};
pub use redaction::{
    DefaultOutputRedactor, MAX_REDACTION_HINT_SECRET_BYTES, MAX_REDACTION_HINT_SECRET_COUNT,
    MAX_REDACTION_HINT_TOTAL_SECRET_BYTES, OutputRedactor, RedactionAudit, RedactionError,
    RedactionHintBoundsError, RedactionHints, RedactionOutcome, RedactionPolicy,
    RedactionPolicyBuilder, RedactionPolicyError, default_redaction_policy,
    default_redaction_policy_dataset_version,
};
