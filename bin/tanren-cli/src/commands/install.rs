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
use tanren_app_services::methodology::RequiredGuard;
use tanren_app_services::methodology::config::{EnvironmentProfile, InstallFormat, TanrenConfig};
use tanren_app_services::methodology::formats::{
    claude_mcp_json, codex_config_toml, opencode_json,
};
use tanren_app_services::methodology::installer::{
    DriftEntry, DriftReason, InstallPlan, PlannedWrite, apply_install, drift,
    plan_install_from_root,
};

const MCP_SERVER_COMMAND: &str = "tanren-mcp";
const MCP_SERVER_ARGS: &[&str] = &["serve"];

/// CLI arguments for `tanren install`.
#[derive(Debug, Args)]
pub(crate) struct InstallArgs {
    /// Path to `tanren.yml`. Defaults to `./tanren.yml`.
    #[arg(long, default_value = "tanren.yml")]
    pub config: PathBuf,

    /// Named profile under `methodology.profiles.<name>` to apply. If
    /// supplied, the profile's keys override the top-level
    /// `methodology` defaults for this install.
    #[arg(long)]
    pub profile: Option<String>,

    /// Override the command source directory. Relative to the repo
    /// root; defaults to the value from `methodology.source.path`.
    #[arg(long)]
    pub source: Option<PathBuf>,

    /// Restrict install to the given target formats (comma-separated).
    /// Accepted values:
    /// `claude-code`, `codex-skills`, `opencode`, `standards-baseline`,
    /// `claude-mcp-json`, `codex-config-toml`, `opencode-json`.
    ///
    /// If supplied, the plan only contains writes whose `format` matches
    /// one of the listed values. If unset, every configured target runs.
    #[arg(long, value_delimiter = ',')]
    pub target: Vec<String>,

    /// Skip writing; print the plan as JSON.
    #[arg(long)]
    pub dry_run: bool,

    /// With `--dry-run`, exit non-zero if the plan diverges from
    /// on-disk state.
    #[arg(long)]
    pub strict: bool,
}

fn parse_target_filter(raw: &[String]) -> Result<Vec<InstallFormat>, String> {
    let mut out = Vec::with_capacity(raw.len());
    for s in raw {
        let fmt = match s.trim() {
            "claude-code" => InstallFormat::ClaudeCode,
            "codex-skills" => InstallFormat::CodexSkills,
            "opencode" => InstallFormat::Opencode,
            "standards-baseline" => InstallFormat::StandardsBaseline,
            "claude-mcp-json" => InstallFormat::ClaudeMcpJson,
            "codex-config-toml" => InstallFormat::CodexConfigToml,
            "opencode-json" => InstallFormat::OpencodeJson,
            other => return Err(format!("unknown --target format `{other}`")),
        };
        if !out.contains(&fmt) {
            out.push(fmt);
        }
    }
    Ok(out)
}

fn phase_hook(env: &EnvironmentProfile, phase: &str) -> Option<String> {
    env.verification_hooks
        .get(phase)
        .cloned()
        .or_else(|| env.verification_hooks.get("default").cloned())
}

fn resolve_primary_verification_hook(
    ctx: &HashMap<String, String>,
    env: &EnvironmentProfile,
    context_key: &str,
    phase_key: &str,
    scoped_legacy_hook: Option<&String>,
    builtin_default: &str,
) -> String {
    ctx.get(context_key)
        .cloned()
        .or_else(|| env.verification_hooks.get(phase_key).cloned())
        .or_else(|| scoped_legacy_hook.cloned())
        .or_else(|| env.verification_hooks.get("default").cloned())
        .or_else(|| env.gate_cmd.clone())
        .unwrap_or_else(|| builtin_default.to_owned())
}

fn resolve_phase_verification_hook(
    ctx: &HashMap<String, String>,
    env: &EnvironmentProfile,
    context_key: &str,
    phase_key: &str,
    fallback: &str,
) -> String {
    ctx.get(context_key)
        .cloned()
        .or_else(|| phase_hook(env, phase_key))
        .unwrap_or_else(|| fallback.to_owned())
}

fn apply_verification_hook_defaults(ctx: &mut HashMap<String, String>, env: &EnvironmentProfile) {
    let task_hook = resolve_primary_verification_hook(
        ctx,
        env,
        "TASK_VERIFICATION_HOOK",
        "do-task",
        env.task_gate_cmd.as_ref(),
        "just check",
    );
    let spec_hook = resolve_primary_verification_hook(
        ctx,
        env,
        "SPEC_VERIFICATION_HOOK",
        "run-demo",
        env.spec_gate_cmd.as_ref(),
        "just ci",
    );
    let audit_task_hook =
        resolve_phase_verification_hook(ctx, env, "AUDIT_TASK_HOOK", "audit-task", &task_hook);
    let adhere_task_hook =
        resolve_phase_verification_hook(ctx, env, "ADHERE_TASK_HOOK", "adhere-task", &task_hook);
    let run_demo_hook =
        resolve_phase_verification_hook(ctx, env, "RUN_DEMO_HOOK", "run-demo", &spec_hook);
    let audit_spec_hook =
        resolve_phase_verification_hook(ctx, env, "AUDIT_SPEC_HOOK", "audit-spec", &spec_hook);
    let adhere_spec_hook =
        resolve_phase_verification_hook(ctx, env, "ADHERE_SPEC_HOOK", "adhere-spec", &spec_hook);
    let demo_hook =
        resolve_phase_verification_hook(ctx, env, "DEMO_HOOK", "run-demo", &run_demo_hook);

    for (key, value) in [
        ("TASK_VERIFICATION_HOOK", task_hook),
        ("SPEC_VERIFICATION_HOOK", spec_hook),
        ("AUDIT_TASK_HOOK", audit_task_hook),
        ("ADHERE_TASK_HOOK", adhere_task_hook),
        ("RUN_DEMO_HOOK", run_demo_hook),
        ("AUDIT_SPEC_HOOK", audit_spec_hook),
        ("ADHERE_SPEC_HOOK", adhere_spec_hook),
        ("DEMO_HOOK", demo_hook),
    ] {
        ctx.entry(key.into()).or_insert(value);
    }
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
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unified_diff: Option<String>,
}

/// Run the install, printing a JSON summary on stdout and returning
/// the intended exit code (0/1/2/3/4).
pub(crate) fn run(args: &InstallArgs) -> u8 {
    let yaml = match std::fs::read_to_string(&args.config) {
        Ok(s) => s,
        Err(e) => return fail_cfg(&format!("reading {}: {e}", args.config.display())),
    };
    let cfg = match TanrenConfig::from_yaml(&yaml) {
        Ok(c) => c,
        Err(e) => return fail_cfg(&format!("parsing {}: {e}", args.config.display())),
    };

    let target_filter = match parse_target_filter(&args.target) {
        Ok(v) => v,
        Err(e) => return fail_validation(&e),
    };

    // Apply `--profile` override. Unknown profile is a hard error — a
    // silent fallback to defaults would hide typos and surprise the
    // caller.
    let mut methodology = cfg.methodology.clone();
    if let Some(name) = args.profile.as_deref() {
        match cfg.methodology.profiles.get(name) {
            Some(profile) => profile.apply(&mut methodology),
            None => {
                return fail_validation(&format!(
                    "unknown --profile `{name}`; known profiles: {:?}",
                    cfg.methodology.profiles.keys().collect::<Vec<_>>()
                ));
            }
        }
    }

    // Apply `--source` override to the command source directory.
    if let Some(src) = args.source.as_ref() {
        methodology.source.path.clone_from(src);
    }

    let pillars =
        match tanren_app_services::methodology::rubric_registry::effective_pillars_for_runtime(
            &args.config,
            &cfg,
        ) {
            Ok(pillars) => pillars,
            Err(err) => return fail_validation(&err),
        };
    let pillar_list = tanren_app_services::methodology::rubric_registry::pillar_ids_csv(&pillars);
    let context = build_context(
        &methodology.variables,
        &methodology.task_complete_requires,
        &pillar_list,
        &cfg.environment.default,
    );

    let mut plan = match plan_install_from_root(&methodology, &context) {
        Ok(p) => p,
        Err(e) => return fail_render(&e.to_string()),
    };

    // Apply `--target` restriction to planned install writes before any
    // MCP config synthesis so non-selected formats remain isolated.
    if !target_filter.is_empty() {
        plan.writes.retain(|w| target_filter.contains(&w.format));
    }

    // Append MCP config writes if configured.
    for cfg_target in &methodology.mcp.also_write_configs {
        if !target_filter.is_empty() && !target_filter.contains(&cfg_target.format) {
            continue;
        }
        match synth_mcp_write(&cfg_target.path, cfg_target.format) {
            Ok(Some(w)) => plan.writes.push(w),
            Ok(None) => {}
            Err(code) => return code,
        }
    }

    let summary = summarize_plan(&plan);
    let drift_list = match drift(&plan) {
        Ok(drift) => drift,
        Err(e) => return fail_io(&e.to_string()),
    };

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
    required_guards: &[RequiredGuard],
    pillar_list: &str,
    env: &EnvironmentProfile,
) -> HashMap<String, String> {
    // Template tokens are `{{UPPERCASE_SNAKE_CASE}}` in the source
    // commands. Normalize every key to upper-case so `tanren.yml`
    // entries can be written in their natural lower-case-with-dots
    // form without worrying about case sensitivity.
    let mut ctx = HashMap::new();
    for (k, v) in user_vars {
        ctx.insert(k.to_ascii_uppercase(), v.clone());
    }

    // Resolve verification-hook variables from config chain:
    // methodology.variables.* -> environment.default.verification_hooks.<phase>
    // -> scoped legacy gate fields -> verification_hooks.default
    // -> gate_cmd -> built-in defaults.
    apply_verification_hook_defaults(&mut ctx, env);

    // Built-in non-hook fallbacks — upper-case so they match `{{VAR}}`
    // substitution directly.
    for (k, v) in [
        ("ISSUE_PROVIDER", "GitHub"),
        ("PROJECT_LANGUAGE", "rust"),
        ("SPEC_ROOT", "tanren/specs"),
        ("PRODUCT_ROOT", "tanren/product"),
        ("STANDARDS_ROOT", "tanren/standards"),
        ("AGENT_CLI_NOUN", "the agent CLI"),
        ("TASK_TOOL_BINDING", "mcp"),
        ("PHASE_EVENTS_FILE", "phase-events.jsonl"),
    ] {
        ctx.entry(k.into()).or_insert_with(|| v.into());
    }
    let required_guards_default = required_guards
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    ctx.entry("REQUIRED_GUARDS".into())
        .or_insert(required_guards_default);
    ctx.entry("PILLAR_LIST".into())
        .or_insert_with(|| pillar_list.to_owned());
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
             spec.md, plan.md, tasks.md, tasks.json, demo.md, and progress.json \
             are generated from the typed event stream. phase-events.jsonl \
             is append-only via typed tools. \
             Postflight reverts unauthorized edits and emits an \
             UnauthorizedArtifactEdit event."
                .into()
        });
    ctx
}

fn synth_mcp_write(path: &Path, format: InstallFormat) -> Result<Option<PlannedWrite>, u8> {
    let existing = std::fs::read_to_string(path).ok();
    let server_args: Vec<String> = MCP_SERVER_ARGS.iter().map(|v| (*v).to_owned()).collect();
    let bytes = match format {
        InstallFormat::ClaudeMcpJson => {
            claude_mcp_json(existing.as_deref(), MCP_SERVER_COMMAND, &server_args)
        }
        InstallFormat::CodexConfigToml => {
            codex_config_toml(existing.as_deref(), MCP_SERVER_COMMAND, &server_args)
        }
        InstallFormat::OpencodeJson => {
            opencode_json(existing.as_deref(), MCP_SERVER_COMMAND, &server_args)
        }
        _ => return Ok(None),
    };
    match bytes {
        Ok(b) => Ok(Some(PlannedWrite {
            dest: path.to_path_buf(),
            bytes: b,
            merge_policy: tanren_app_services::methodology::config::MergePolicy::PreserveOtherKeys,
            format,
        })),
        Err(e) => Err(fail_render(&e.to_string())),
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
    match &d.reason {
        DriftReason::Missing => DriftSummary {
            dest: d.dest.clone(),
            kind: "missing".into(),
            expected_sha256: None,
            actual_sha256: None,
            unified_diff: None,
        },
        DriftReason::ExtraFile => DriftSummary {
            dest: d.dest.clone(),
            kind: "extra_file".into(),
            expected_sha256: None,
            actual_sha256: None,
            unified_diff: None,
        },
        DriftReason::Differs(diff) => DriftSummary {
            dest: d.dest.clone(),
            kind: "differs".into(),
            expected_sha256: Some(diff.expected_sha256.clone()),
            actual_sha256: Some(diff.actual_sha256.clone()),
            unified_diff: Some(diff.unified_diff.clone()),
        },
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
fn fail_render(msg: &str) -> u8 {
    emit_err_json("render_error", msg);
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
