use std::path::PathBuf;

use tanren_configuration_secrets::ConfigSecretsError;
use tanren_contract::{
    EffectiveConfigurationActorUsability, EffectiveConfigurationFreshness,
    EffectiveConfigurationPolicyConstraint, EffectiveConfigurationResolutionKind,
    EffectiveConfigurationSettingFamily, EffectiveConfigurationSourceScope,
};
use thiserror::Error;

use tanren_testkit::{HarnessError, InstallProofError};

#[derive(Debug, Error)]
pub(crate) enum InstallStepError {
    #[error("failed to create install scenario repository fixture directory `{path}`: {source}")]
    CreateFixtureDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to remove install scenario repository fixture directory `{path}`: {source}")]
    RemoveFixtureDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to execute tanren-cli install via CLI harness adapter: {source}")]
    RunInstallCommand { source: HarnessError },
    #[error("failed to execute tanren-cli standards inspect via CLI harness adapter: {source}")]
    RunStandardsInspectCommand { source: HarnessError },
    #[error("install command has not been executed yet")]
    InstallCommandNotExecuted,
    #[error("install command must run before no-write assertion")]
    MissingSnapshotBeforeRun,
    #[error("baseline must be recorded before assertion for `{path}`")]
    MissingBaseline { path: String },
    #[error(
        "expected install command to succeed; status={status:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    InstallCommandExpectedSuccess {
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error(
        "expected install command to fail with non-zero exit; stdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    InstallCommandExpectedFailure { stdout: String, stderr: String },
    #[error("expected install stdout to contain `{expected}`; got:\n{stdout}")]
    StdoutMissingExpected { expected: String, stdout: String },
    #[error(
        "expected install stdout to decode as standards inspect JSON report: {source}\nstdout:\n{stdout}"
    )]
    StdoutJsonDecode {
        source: serde_json::Error,
        stdout: String,
    },
    #[error(
        "expected install stdout standards inspect JSON report to include field path `{field_path}`\nstdout:\n{stdout}"
    )]
    StdoutJsonMissingField { field_path: String, stdout: String },
    #[error("expected install stderr to contain `{expected}`; got:\n{stderr}")]
    StderrMissingExpected { expected: String, stderr: String },
    #[error(
        "install command output leaked absolute repository path `{path}`\nstdout:\n{stdout}\nstderr:\n{stderr}"
    )]
    OutputLeakedAbsoluteRepositoryPath {
        path: String,
        stdout: String,
        stderr: String,
    },
    #[error("expected validation failure in stderr; got:\n{stderr}")]
    ValidationFailureMissing { stderr: String },
    #[error("unexpected standards inspect status `{actual}` in success report")]
    UnexpectedStandardsInspectStatus { actual: String },
    #[error("unexpected standards inspect command `{actual}` in success report")]
    UnexpectedStandardsInspectCommand { actual: String },
    #[error("unexpected standards inspect profile; expected `{expected}`, got `{actual}`")]
    UnexpectedStandardsInspectProfile { expected: String, actual: String },
    #[error("unexpected standards inspect standards_root; expected `{expected}`, got `{actual}`")]
    UnexpectedStandardsInspectStandardsRoot { expected: String, actual: String },
    #[error("standards inspect report returned an empty repository field")]
    UnexpectedStandardsInspectRepositoryEmpty,
    #[error("standards inspect report returned an empty standards_root")]
    UnexpectedStandardsInspectStandardsRootEmpty,
    #[error("standards inspect report returned standards_count=0")]
    UnexpectedStandardsInspectCountZero,
    #[error("standards inspect report returned an empty first_standard_name")]
    UnexpectedStandardsInspectFirstStandardNameEmpty,
    #[error(
        "standards inspect report first_standard_path `{path}` is outside configured standards_root `{standards_root}`"
    )]
    UnexpectedStandardsInspectFirstStandardPathOutsideStandardsRoot {
        path: String,
        standards_root: String,
    },
    #[error(
        "unexpected effective-configuration setting_family; expected `{expected:?}`, got `{actual:?}`"
    )]
    UnexpectedEffectiveConfigurationSettingFamily {
        expected: EffectiveConfigurationSettingFamily,
        actual: EffectiveConfigurationSettingFamily,
    },
    #[error("unexpected effective-configuration source_scope `{actual:?}`")]
    UnexpectedEffectiveConfigurationSourceScope {
        actual: EffectiveConfigurationSourceScope,
    },
    #[error("unexpected effective-configuration resolution_kind `{actual:?}`")]
    UnexpectedEffectiveConfigurationResolutionKind {
        actual: EffectiveConfigurationResolutionKind,
    },
    #[error("unexpected effective-configuration policy_constraint `{actual:?}`")]
    UnexpectedEffectiveConfigurationPolicyConstraint {
        actual: EffectiveConfigurationPolicyConstraint,
    },
    #[error("unexpected effective-configuration actor_usability `{actual:?}`")]
    UnexpectedEffectiveConfigurationActorUsability {
        actual: EffectiveConfigurationActorUsability,
    },
    #[error("unexpected effective-configuration freshness `{actual:?}`")]
    UnexpectedEffectiveConfigurationFreshness {
        actual: EffectiveConfigurationFreshness,
    },
    #[error("expected repository fixture to remain unchanged after command")]
    RepositorySnapshotMismatch,
    #[error("expected stale path to be absent before manifest injection: {path}")]
    StaleManifestPathAlreadyPresent { path: String },
    #[error("expected repository file content to equal baseline for `{path}`")]
    FileContentChanged { path: PathBuf },
    #[error("expected repository file content to differ from baseline for `{path}`")]
    FileContentNotReplaced { path: PathBuf },
    #[error("unexpected repository file content for `{path}`")]
    UnexpectedFileContent { path: PathBuf },
    #[error("expected repository file to exist: {path}")]
    ExpectedFileToExist { path: PathBuf },
    #[error("expected repository file to be absent: {path}")]
    ExpectedFileToBeAbsent { path: PathBuf },
    #[error("expected repository directory to exist: {path}")]
    ExpectedDirectoryToExist { path: PathBuf },
    #[error("no markdown standards files found under configured standards root '{path}'")]
    StandardsMarkdownFileMissing { path: PathBuf },
    #[error("failed to parse project methodology config '{path}': {source}")]
    ParseProjectMethodologyConfig {
        path: PathBuf,
        source: ConfigSecretsError,
    },
    #[error("failed to serialize project methodology config '{path}': {source}")]
    SerializeProjectMethodologyConfig {
        path: PathBuf,
        source: ConfigSecretsError,
    },
    #[error("invalid standards root '{path}' for project methodology config: {source}")]
    InvalidStandardsRootForConfig {
        path: String,
        source: ConfigSecretsError,
    },
    #[error("repository-relative path cannot be empty")]
    EmptyRepositoryRelativePath,
    #[error("repository-relative path must not be absolute: {path}")]
    AbsoluteRepositoryRelativePath { path: String },
    #[error("repository-relative path must not contain traversal components: {path}")]
    TraversalRepositoryRelativePath { path: String },
    #[error("repository-relative path failed delivery path contract validation: {path}")]
    InstallPathContractRejected { path: String },
    #[error("delivery-owned install proof assertion failed: {source}")]
    InstallProofFailure { source: InstallProofError },
    #[error("failed to read directory `{path}`: {source}")]
    ReadDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to inspect directory entry in `{path}`: {source}")]
    ReadDirectoryEntry {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to inspect file type for `{path}`: {source}")]
    InspectFileType {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("path `{path}` must stay under root `{root}`: {source}")]
    PathOutsideRoot {
        path: PathBuf,
        root: PathBuf,
        source: std::path::StripPrefixError,
    },
    #[error("failed to read file `{path}` while attempting to {action}: {source}")]
    ReadFile {
        path: PathBuf,
        action: &'static str,
        source: std::io::Error,
    },
    #[error("failed to write file `{path}` while attempting to {action}: {source}")]
    WriteFile {
        path: PathBuf,
        action: &'static str,
        source: std::io::Error,
    },
    #[error("filesystem operation `{action}` failed at `{path}`: {source}")]
    Io {
        path: PathBuf,
        action: &'static str,
        source: std::io::Error,
    },
    #[error(
        "install context unavailable; install steps must run under explicit @cli scenario dispatch"
    )]
    InstallContextUnavailable,
    #[error(
        "account harness context unavailable; scenario before-hook dispatch did not initialize"
    )]
    AccountContextUnavailable,
}

pub(crate) type InstallStepResult<T> = Result<T, InstallStepError>;
