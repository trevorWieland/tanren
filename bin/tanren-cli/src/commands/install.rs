//! `tanren install` — bootstrap a repo from `tanren.yml`.
//!
//! Renders the command catalog + bundled standards per the
//! `methodology` section of `tanren.yml` and writes them to each
//! configured target (`Claude Code`, `Codex Skills`, `OpenCode`,
//! `tanren/standards/`).
//!
//! Exit codes:
//!
//! - 0: apply or dry-run completed without drift
//! - 1: config or render error
//! - 2: I/O / write error
//! - 3: `--strict --dry-run` found drift
//! - 4: validation error

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clap::Args;
use serde::Serialize;
use tanren_app_services::methodology::config::{InstallFormat, TanrenConfig};
use tanren_app_services::methodology::formats::{
    claude_mcp_json, codex_config_toml, opencode_json,
};
use tanren_app_services::methodology::installer::{
    DriftEntry, InstallPlan, PlannedWrite, apply_install, drift, plan_install_from_root,
};

/// CLI arguments for `tanren install`.
#[derive(Debug, Args)]
pub(crate) struct InstallArgs {
    /// Path to `tanren.yml`. Defaults to `./tanren.yml`.
    #[arg(long, default_value = "tanren.yml")]
    pub config: PathBuf,

    /// Skip writing; print the plan as JSON.
    #[arg(long)]
    pub dry_run: bool,

    /// With `--dry-run`, exit non-zero if the plan diverges from
    /// on-disk state.
    #[arg(long)]
    pub strict: bool,
}

/// Structured output emitted on successful install / dry-run.
#[derive(Debug, Serialize)]
pub(crate) struct InstallOutcome {
    pub dry_run: bool,
    pub strict: bool,
    pub planned: Vec<PlannedSummary>,
    pub written: Vec<PathBuf>,
    pub drift: Vec<DriftSummary>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PlannedSummary {
    pub dest: PathBuf,
    pub format: String,
    pub merge_policy: String,
    pub bytes: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct DriftSummary {
    pub dest: PathBuf,
    pub reason: String,
}

/// Run the install, printing a JSON summary on stdout and returning
/// the intended exit code (0/1/2/3/4).
pub(crate) fn run(args: &InstallArgs) -> u8 {
    let yaml = match std::fs::read_to_string(&args.config) {
        Ok(s) => s,
        Err(e) => return fail_io(&format!("reading {}: {e}", args.config.display())),
    };
    let cfg = match TanrenConfig::from_yaml(&yaml) {
        Ok(c) => c,
        Err(e) => return fail_cfg(&format!("parsing {}: {e}", args.config.display())),
    };

    let context = build_context(&cfg.methodology.variables);

    let mut plan = match plan_install_from_root(&cfg.methodology, &context) {
        Ok(p) => p,
        Err(e) => return fail_validation(&e.to_string()),
    };

    // Append MCP config writes if configured.
    for cfg_target in &cfg.methodology.mcp.also_write_configs {
        match synth_mcp_write(&cfg_target.path, cfg_target.format) {
            Ok(Some(w)) => plan.writes.push(w),
            Ok(None) => {}
            Err(code) => return code,
        }
    }

    let summary = summarize_plan(&plan);
    let drift_list = drift(&plan);

    if args.dry_run {
        let outcome = InstallOutcome {
            dry_run: true,
            strict: args.strict,
            planned: summary,
            written: vec![],
            drift: drift_list.iter().map(drift_summary).collect(),
        };
        emit_json(&outcome);
        return if args.strict && !drift_list.is_empty() {
            3
        } else {
            0
        };
    }

    let written = match apply_install(&plan) {
        Ok(w) => w,
        Err(e) => return fail_io(&e.to_string()),
    };
    let outcome = InstallOutcome {
        dry_run: false,
        strict: args.strict,
        planned: summary,
        written,
        drift: vec![],
    };
    emit_json(&outcome);
    0
}

fn build_context(
    user_vars: &std::collections::BTreeMap<String, String>,
) -> HashMap<String, String> {
    // Template tokens are `{{UPPERCASE_SNAKE_CASE}}` in the source
    // commands. Normalize every key to upper-case so `tanren.yml`
    // entries can be written in their natural lower-case-with-dots
    // form without worrying about case sensitivity.
    let mut ctx = HashMap::new();
    for (k, v) in user_vars {
        ctx.insert(k.to_ascii_uppercase(), v.clone());
    }
    // Built-in fallbacks — upper-case so they match `{{VAR}}`
    // substitution directly.
    for (k, v) in [
        ("TASK_VERIFICATION_HOOK", "just check"),
        ("SPEC_VERIFICATION_HOOK", "just ci"),
        ("ISSUE_PROVIDER", "GitHub"),
        ("PROJECT_LANGUAGE", "rust"),
        ("SPEC_ROOT", "tanren/specs"),
        ("PRODUCT_ROOT", "tanren/product"),
        ("STANDARDS_ROOT", "tanren/standards"),
        ("AGENT_CLI_NOUN", "the agent CLI"),
        ("TASK_TOOL_BINDING", "mcp"),
        ("PHASE_EVENTS_FILE", "phase-events.jsonl"),
        ("ADHERE_SPEC_HOOK", "just check"),
        ("ADHERE_TASK_HOOK", "just check"),
        ("AUDIT_SPEC_HOOK", "just check"),
        ("AUDIT_TASK_HOOK", "just check"),
        ("DEMO_HOOK", "just check"),
        ("RUN_DEMO_HOOK", "just check"),
        (
            "PILLAR_LIST",
            "completeness, performance, scalability, strictness, security, \
             stability, maintainability, extensibility, elegance, style, \
             relevance, modularity, documentation_complete",
        ),
        ("REQUIRED_GUARDS", "gate_checked, audited, adherent"),
    ] {
        ctx.entry(k.into()).or_insert_with(|| v.into());
    }
    // Derived defaults — noun phrases for the chosen issue provider.
    let issue_provider = ctx
        .get("ISSUE_PROVIDER")
        .cloned()
        .unwrap_or_else(|| "GitHub".into());
    let (issue_ref_noun, pr_noun) = match issue_provider.to_ascii_lowercase().as_str() {
        "linear" => ("Linear issue", "merge request"),
        _ => ("GitHub issue", "pull request"),
    };
    ctx.entry("ISSUE_REF_NOUN".into())
        .or_insert_with(|| issue_ref_noun.into());
    ctx.entry("PR_NOUN".into())
        .or_insert_with(|| pr_noun.into());
    ctx.entry("READONLY_ARTIFACT_BANNER".into())
        .or_insert_with(|| {
            "⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT: \
             plan.md and progress.json are generated from the typed task \
             store. Postflight reverts unauthorized edits and emits an \
             UnauthorizedArtifactEdit event."
                .into()
        });
    ctx
}

fn synth_mcp_write(path: &Path, format: InstallFormat) -> Result<Option<PlannedWrite>, u8> {
    let existing = std::fs::read_to_string(path).ok();
    let bytes = match format {
        InstallFormat::ClaudeMcpJson => claude_mcp_json(existing.as_deref(), "tanren-mcp", &[]),
        InstallFormat::CodexConfigToml => codex_config_toml(existing.as_deref(), "tanren-mcp", &[]),
        InstallFormat::OpencodeJson => opencode_json(existing.as_deref(), "tanren-mcp", &[]),
        _ => return Ok(None),
    };
    match bytes {
        Ok(b) => Ok(Some(PlannedWrite {
            dest: path.to_path_buf(),
            bytes: b,
            merge_policy: tanren_app_services::methodology::config::MergePolicy::PreserveOtherKeys,
            format,
        })),
        Err(e) => Err(fail_validation(&e.to_string())),
    }
}

fn summarize_plan(plan: &InstallPlan) -> Vec<PlannedSummary> {
    plan.writes
        .iter()
        .map(|w| PlannedSummary {
            dest: w.dest.clone(),
            format: format!("{:?}", w.format),
            merge_policy: format!("{:?}", w.merge_policy),
            bytes: w.bytes.len(),
        })
        .collect()
}

fn drift_summary(d: &DriftEntry) -> DriftSummary {
    DriftSummary {
        dest: d.dest.clone(),
        reason: format!("{:?}", d.reason),
    }
}

fn emit_json<T: Serialize>(value: &T) {
    // stdout is the right place for structured install output; stderr
    // is reserved for logs + errors.
    if let Ok(s) = serde_json::to_string_pretty(value) {
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout(), "{s}");
    }
}

fn emit_err_json(kind: &str, msg: &str) {
    use std::io::Write as _;
    let payload = serde_json::json!({ "error": kind, "message": msg });
    if let Ok(s) = serde_json::to_string_pretty(&payload) {
        let _ = writeln!(std::io::stderr(), "{s}");
    }
}

fn fail_cfg(msg: &str) -> u8 {
    emit_err_json("config_error", msg);
    1
}
fn fail_io(msg: &str) -> u8 {
    emit_err_json("io_error", msg);
    2
}
fn fail_validation(msg: &str) -> u8 {
    emit_err_json("validation_error", msg);
    4
}
