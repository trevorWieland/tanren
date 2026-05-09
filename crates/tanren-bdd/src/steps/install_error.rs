use std::path::PathBuf;

use thiserror::Error;

use tanren_cli_app::install::InstallError;
use tanren_testkit::{HarnessError, HarnessKind};

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
    #[error("failed to {action} for install plan/apply scenario: {source}")]
    InstallPlanOperation {
        action: &'static str,
        source: InstallError,
    },
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
    #[error("expected install stderr to contain `{expected}`; got:\n{stderr}")]
    StderrMissingExpected { expected: String, stderr: String },
    #[error("expected validation failure in stderr; got:\n{stderr}")]
    ValidationFailureMissing { stderr: String },
    #[error("expected repository fixture to remain unchanged after command")]
    RepositorySnapshotMismatch,
    #[error("expected install plan/apply failure, but apply succeeded")]
    PreparedPlanApplyUnexpectedSuccess,
    #[error("expected a prepared install plan before apply")]
    MissingPreparedInstallPlan,
    #[error("expected prepared install plan apply to fail, but no apply error was recorded")]
    PreparedPlanApplyDidNotFail,
    #[error(
        "expected unsafe repository path exactly `{expected}` from prepared plan apply; got `{actual}`"
    )]
    UnsafeRepositoryPathMismatch { expected: String, actual: String },
    #[error(
        "expected unsafe repository path starting with `{expected_prefix}` from prepared plan apply; got `{actual}`"
    )]
    UnsafeRepositoryPathPrefixMismatch {
        expected_prefix: String,
        actual: String,
    },
    #[error(
        "expected unsafe repository path failure from prepared plan apply; got `{actual_error}`"
    )]
    PreparedPlanExpectedUnsafeRepositoryPath { actual_error: String },
    #[error("expected prepared plan apply failure message to contain `{expected}`; got `{actual}`")]
    PreparedPlanFailureMessageMissing { expected: String, actual: String },
    #[error(
        "expected prepared install plan to include a path starting with `{expected_prefix}` in {action}"
    )]
    PlannedPathPrefixMissing {
        expected_prefix: String,
        action: &'static str,
    },
    #[error("expected prepared install plan to include path `{expected}` in {action}")]
    PlannedPathMissing {
        expected: String,
        action: &'static str,
    },
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
    #[error("repository-relative path cannot be empty")]
    EmptyRepositoryRelativePath,
    #[error("repository-relative path must not be absolute: {path}")]
    AbsoluteRepositoryRelativePath { path: String },
    #[error("repository-relative path must not contain traversal components: {path}")]
    TraversalRepositoryRelativePath { path: String },
    #[error("unsupported integration in BDD assertion: {integration}")]
    UnsupportedIntegrationForAssertion { integration: String },
    #[error("expected workspace catalog root to exist: {path}")]
    MissingCatalogRoot { path: PathBuf },
    #[error("expected workspace catalog `{catalog_root}` to have at least one file")]
    CatalogEmpty { catalog_root: &'static str },
    #[error("failed to canonicalize workspace root for BDD install steps: {source}")]
    CanonicalizeWorkspaceRoot { source: std::io::Error },
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
    #[error("install manifest `{manifest_path}` is missing `{expected}`\nmanifest:\n{manifest}")]
    ManifestMissingContent {
        expected: String,
        manifest_path: PathBuf,
        manifest: String,
    },
    #[error("install context should be available after initialization")]
    InstallContextUnavailable,
    #[error(
        "account harness context unavailable; scenario before-hook dispatch did not initialize"
    )]
    AccountContextUnavailable,
    #[error(
        "install steps require an active @cli harness from tag dispatch; got {actual:?} harness"
    )]
    InstallRequiresCliHarness { actual: HarnessKind },
}

pub(crate) type InstallStepResult<T> = Result<T, InstallStepError>;
