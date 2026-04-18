//! Methodology CLI subcommands.
//!
//! The CLI is the 1:1 fallback transport for the methodology tool
//! surface. Every tool in `agent-tool-surface.md §3` is reachable via
//! a `tanren <noun> <verb>` subcommand, accepting JSON-encoded
//! parameters that exactly match the wire contract in
//! `tanren-contract::methodology`. Both MCP and CLI call the same
//! `MethodologyService` methods so event trails are byte-identical.
//!
//! ## Input shape
//!
//! Every subcommand reads a JSON-encoded contract params struct from
//! one of three sources, in priority order:
//!
//! 1. `--json '{ … }'` inline on the command line.
//! 2. `--params-file <PATH>` pointing at a file containing JSON.
//! 3. `--params-stdin` reading JSON from stdin.
//!
//! ## Phase + capability resolution
//!
//! Capability enforcement mirrors the MCP transport: we consult the
//! `TANREN_PHASE_CAPABILITIES` env var (comma-separated capability
//! tags). When unset, we fall back to the `--phase`-keyed default
//! scope from `default_scope_for_phase`. When neither yields a scope
//! (unknown phase, no env), the CLI *grants all capabilities* — this
//! administrative mode is intentional and documented; CI scripts and
//! the orchestrator both set an explicit env before invoking the
//! CLI.

pub(crate) mod adherence;
pub(crate) mod demo;
pub(crate) mod finding;
pub(crate) mod ingest;
pub(crate) mod issue;
pub(crate) mod phase;
pub(crate) mod replay;
pub(crate) mod rubric;
pub(crate) mod signpost;
pub(crate) mod spec;
pub(crate) mod standard;
pub(crate) mod task;

use std::io::{Read as _, Write as _};
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use clap::{Args, Subcommand};
use serde::{Serialize, de::DeserializeOwned};
use tanren_app_services::methodology::{
    CapabilityScope, MethodologyError, MethodologyService, ToolCapability, ToolError,
    default_scope_for_phase, parse_scope_env,
};

/// Top-level arguments shared by every methodology subcommand.
#[derive(Debug, Clone, Args)]
pub(crate) struct MethodologyGlobal {
    /// Phase name used for capability enforcement and audit trail.
    ///
    /// Defaults to `cli-admin`, which is not a registered phase and
    /// so falls back to the "all capabilities" administrative mode
    /// unless `TANREN_PHASE_CAPABILITIES` is set.
    #[arg(long, global = true, default_value = "cli-admin")]
    pub phase: String,
}

/// Shape of every methodology subcommand's input-source flags.
///
/// Exactly one of `--json`, `--params-file`, `--params-stdin` must
/// be supplied. The validation happens at load time rather than via
/// clap groups so we can produce a typed `ToolError` on misuse.
#[derive(Debug, Clone, Args)]
pub(crate) struct ParamsInput {
    /// Inline JSON params string.
    #[arg(long, conflicts_with_all = ["params_file", "params_stdin"])]
    pub json: Option<String>,

    /// Read JSON params from a file path.
    #[arg(long, conflicts_with_all = ["json", "params_stdin"])]
    pub params_file: Option<PathBuf>,

    /// Read JSON params from stdin.
    #[arg(long, default_value_t = false, conflicts_with_all = ["json", "params_file"])]
    pub params_stdin: bool,
}

/// Every methodology-command family (one enum variant per noun).
#[derive(Debug, Subcommand)]
pub(crate) enum MethodologyCommand {
    /// Task lifecycle.
    #[command(subcommand)]
    Task(task::TaskCommand),
    /// Findings.
    #[command(subcommand)]
    Finding(finding::FindingCommand),
    /// Rubric / non-negotiable compliance.
    #[command(subcommand)]
    Rubric(rubric::RubricCommand),
    /// Spec frontmatter.
    #[command(subcommand)]
    Spec(spec::SpecCommand),
    /// Demo frontmatter.
    #[command(subcommand)]
    Demo(demo::DemoCommand),
    /// Signposts.
    #[command(subcommand)]
    Signpost(signpost::SignpostCommand),
    /// Phase lifecycle.
    #[command(subcommand)]
    Phase(phase::PhaseCommand),
    /// Backlog issues.
    #[command(subcommand)]
    Issue(issue::IssueCommand),
    /// Standards read.
    #[command(subcommand)]
    Standard(standard::StandardCommand),
    /// Adherence findings.
    #[command(subcommand)]
    Adherence(adherence::AdherenceCommand),
    /// Ingest a phase-events.jsonl file into the store.
    IngestPhaseEvents(ingest::IngestArgs),
    /// Replay a spec folder's recorded phase events into the store.
    Replay(replay::ReplayArgs),
}

/// Load JSON params from the configured input source and deserialize
/// into `T`.
///
/// # Errors
/// Returns `MethodologyError::FieldValidation` when the input source
/// is missing or empty; `MethodologyError::Validation` on JSON
/// decode failure.
pub(crate) fn load_params<T: DeserializeOwned>(input: &ParamsInput) -> Result<T, MethodologyError> {
    let raw = if let Some(j) = &input.json {
        j.clone()
    } else if let Some(p) = &input.params_file {
        std::fs::read_to_string(p).map_err(|source| MethodologyError::Io {
            path: p.clone(),
            source,
        })?
    } else if input.params_stdin {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|source| MethodologyError::Io {
                path: PathBuf::from("<stdin>"),
                source,
            })?;
        buf
    } else {
        return Err(MethodologyError::FieldValidation {
            field_path: "/params".into(),
            expected: "one of --json, --params-file, --params-stdin".into(),
            actual: "no input source supplied".into(),
            remediation: "pass JSON via --json '<...>', --params-file <PATH>, or --params-stdin"
                .into(),
        });
    };
    serde_json::from_str::<T>(&raw).map_err(|e| MethodologyError::FieldValidation {
        field_path: format!("/params (line {} col {})", e.line(), e.column()),
        expected: std::any::type_name::<T>().to_owned(),
        actual: e.to_string(),
        remediation: "ensure the JSON body matches the tool's contract shape".into(),
    })
}

/// Resolve the capability scope in effect for this invocation.
///
/// Precedence (zero-trust default-deny):
/// 1. `TANREN_PHASE_CAPABILITIES` env var if set — parsed explicit scope.
/// 2. `default_scope_for_phase(phase)` if the phase has a documented
///    capability set.
/// 3. `TANREN_CAPABILITY_OVERRIDE=admin` explicit override — grants
///    every capability, logged at `warn` level so audit trails capture
///    each use.
/// 4. Default deny: return an empty scope so downstream
///    `enforce(..)` calls surface a typed `CapabilityDenied`.
#[must_use]
pub(crate) fn resolve_scope(phase: &str) -> CapabilityScope {
    if let Ok(env) = std::env::var("TANREN_PHASE_CAPABILITIES")
        && !env.trim().is_empty()
    {
        return parse_scope_env(&env);
    }
    if let Some(scope) = default_scope_for_phase(phase) {
        return scope;
    }
    if matches!(
        std::env::var("TANREN_CAPABILITY_OVERRIDE").as_deref(),
        Ok("admin")
    ) {
        tracing::warn!(
            phase,
            "admin capability override in use — TANREN_CAPABILITY_OVERRIDE=admin grants full tool scope"
        );
        return all_capabilities_scope();
    }
    CapabilityScope::from_iter_caps([])
}

fn all_capabilities_scope() -> CapabilityScope {
    use ToolCapability::{
        AdherenceRecord, ComplianceRecord, DemoFrontmatter, DemoResults, FeedbackReply, FindingAdd,
        IssueCreate, PhaseEscalate, PhaseOutcome, RubricRecord, SignpostAdd, SignpostUpdate,
        SpecFrontmatter, StandardRead, TaskAbandon, TaskComplete, TaskCreate, TaskRead, TaskRevise,
        TaskStart,
    };
    CapabilityScope::from_iter_caps([
        TaskCreate,
        TaskStart,
        TaskComplete,
        TaskRevise,
        TaskAbandon,
        TaskRead,
        FindingAdd,
        RubricRecord,
        ComplianceRecord,
        SpecFrontmatter,
        DemoFrontmatter,
        DemoResults,
        SignpostAdd,
        SignpostUpdate,
        PhaseOutcome,
        PhaseEscalate,
        IssueCreate,
        StandardRead,
        AdherenceRecord,
        FeedbackReply,
    ])
}

/// Render a methodology result to stdout (success → JSON response;
/// error → typed `ToolError` on stderr + non-zero exit code). Returns
/// the raw exit byte so `main` can route through `RunError::TypedExit`.
pub(crate) fn emit_result<R: Serialize>(result: Result<R, MethodologyError>) -> u8 {
    match result {
        Ok(response) => {
            if let Err(e) = write_json_stdout(&response) {
                let _ = writeln!(std::io::stderr(), "{e}");
                return 2;
            }
            0
        }
        Err(err) => {
            let code = exit_code_for(&err);
            let tool_err: ToolError = (&err).into();
            if let Ok(json) = serde_json::to_string_pretty(&tool_err) {
                let _ = writeln!(std::io::stderr(), "{json}");
            }
            code
        }
    }
}

/// Convert a methodology error into the CLI's typed exit code.
/// Aligns with the installer's 0/1/2/3/4 contract: validation = 4,
/// I/O = 2, everything else = 1.
#[must_use]
pub(crate) fn exit_code_for(err: &MethodologyError) -> u8 {
    match err {
        MethodologyError::FieldValidation { .. }
        | MethodologyError::Validation(_)
        | MethodologyError::CapabilityDenied { .. }
        | MethodologyError::IllegalTaskTransition { .. }
        | MethodologyError::NotFound { .. }
        | MethodologyError::Conflict { .. }
        | MethodologyError::EvidenceSchema { .. } => 4,
        MethodologyError::Io { .. }
        | MethodologyError::Domain(_)
        | MethodologyError::Store(_)
        | MethodologyError::Projection(_) => 2,
        // Typed replay/ingest errors preserve their structured shape
        // through the CLI boundary so operators can machine-read
        // `{ code, line, raw }` without string parsing.
        MethodologyError::ReplayMalformedLine { .. } => 5,
        MethodologyError::ReplayEnvelopeDecode { .. } => 6,
        MethodologyError::Internal(_) => 1,
    }
}

fn write_json_stdout<R: Serialize>(value: &R) -> Result<()> {
    let json = serde_json::to_string_pretty(value).context("serialize response")?;
    writeln!(std::io::stdout(), "{json}").context("write stdout")?;
    Ok(())
}

/// Run a subcommand: load JSON params, call the service, emit result.
pub(crate) async fn dispatch(
    service: &MethodologyService,
    global: &MethodologyGlobal,
    command: MethodologyCommand,
) -> u8 {
    let scope = resolve_scope(&global.phase);
    let phase = global.phase.clone();
    match command {
        MethodologyCommand::Task(c) => task::run(service, &scope, &phase, c).await,
        MethodologyCommand::Finding(c) => finding::run(service, &scope, &phase, c).await,
        MethodologyCommand::Rubric(c) => rubric::run(service, &scope, &phase, c).await,
        MethodologyCommand::Spec(c) => spec::run(service, &scope, &phase, c).await,
        MethodologyCommand::Demo(c) => demo::run(service, &scope, &phase, c).await,
        MethodologyCommand::Signpost(c) => signpost::run(service, &scope, &phase, c).await,
        MethodologyCommand::Phase(c) => phase::run(service, &scope, &phase, c).await,
        MethodologyCommand::Issue(c) => issue::run(service, &scope, &phase, c).await,
        MethodologyCommand::Standard(c) => standard::run(service, &scope, &phase, c),
        MethodologyCommand::Adherence(c) => adherence::run(service, &scope, &phase, c).await,
        MethodologyCommand::IngestPhaseEvents(a) => ingest::run(service, a).await,
        MethodologyCommand::Replay(a) => replay::run(service, a).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_validation_is_four() {
        let e = MethodologyError::FieldValidation {
            field_path: "/title".into(),
            expected: "non-empty".into(),
            actual: "\"\"".into(),
            remediation: "supply a title".into(),
        };
        assert_eq!(exit_code_for(&e), 4);
    }

    #[test]
    fn exit_code_io_is_two() {
        let e = MethodologyError::Io {
            path: PathBuf::from("/tmp/x"),
            source: std::io::Error::other("x"),
        };
        assert_eq!(exit_code_for(&e), 2);
    }

    #[test]
    fn resolve_scope_unknown_phase_defaults_deny_without_override() {
        // Audit-remediation: the CLI now defaults to zero capabilities
        // when neither `TANREN_PHASE_CAPABILITIES` nor the explicit
        // `TANREN_CAPABILITY_OVERRIDE=admin` opt-in is set. Avoid env
        // mutation here — see the admin-override integration path for
        // the positive case.
        if std::env::var("TANREN_PHASE_CAPABILITIES").is_ok()
            || std::env::var("TANREN_CAPABILITY_OVERRIDE").is_ok()
        {
            return;
        }
        let scope = resolve_scope("cli-admin");
        assert!(!scope.allows(ToolCapability::TaskCreate));
        assert!(!scope.allows(ToolCapability::PhaseEscalate));
    }
}
