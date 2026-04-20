//! `tanren-mcp` — Model Context Protocol server exposing the
//! methodology tool surface over stdio.
//!
//! Lane 0.5 scope:
//!
//! - stdio transport only (rmcp `transport-io` feature).
//! - Capability scope + phase/spec/session derived from a signed
//!   `TANREN_MCP_CAPABILITY_ENVELOPE`.
//! - Database URL from `TANREN_DATABASE_URL` (default
//!   `"sqlite:tanren.db?mode=rwc"`) — the same store path the CLI
//!   uses, so event trails are byte-identical across transports.
//! - Every tool dispatched through
//!   `tanren_app_services::methodology::MethodologyService`.
//! - `tracing` writes to **stderr** only (non-negotiable #14 — stdout
//!   is reserved for MCP framing).
#![deny(clippy::disallowed_types, clippy::disallowed_methods)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use tracing_subscriber::EnvFilter;

mod catalog;
mod dispatch;
mod handler;
mod scope;
mod tool_registry;

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(err) = init_tracing() {
        let _ = writeln_stderr(&format!("failed to initialize tracing: {err}"));
        return ExitCode::from(2);
    }
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(?err, "tanren-mcp exited with error");
            ExitCode::from(1)
        }
    }
}

async fn run() -> Result<()> {
    let envelope = scope::verify_from_env().context("verifying signed MCP capability envelope")?;
    let database_url = std::env::var("TANREN_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:tanren.db?mode=rwc".to_owned());
    let config_path = std::env::var("TANREN_CONFIG").unwrap_or_else(|_| "tanren.yml".to_owned());
    let runtime = load_methodology_runtime_settings(Path::new(&config_path))
        .with_context(|| format!("loading methodology config from {config_path}"))?;
    let standards = tanren_app_services::methodology::standards::load_runtime_standards(
        &runtime.standards_root,
    )
    .with_context(|| {
        format!(
            "loading standards from {}",
            runtime.standards_root.display()
        )
    })?;
    let phase_events = std::env::var("TANREN_SPEC_FOLDER").ok().and_then(|raw| {
        let spec_folder = raw.trim();
        if spec_folder.is_empty() {
            None
        } else {
            Some(
                tanren_app_services::methodology::service::PhaseEventsRuntime {
                    spec_id: envelope.spec_id,
                    spec_folder: PathBuf::from(spec_folder),
                    agent_session_id: envelope.agent_session_id.clone(),
                },
            )
        }
    });
    let reconcile_spec_folder = phase_events
        .as_ref()
        .map(|runtime| runtime.spec_folder.clone());
    let service = tanren_app_services::compose::build_methodology_service_with_config(
        &database_url,
        runtime.required_guards,
        phase_events,
        standards,
        runtime.pillars,
        runtime.issue_provider,
    )
    .await
    .context("building methodology service")?;
    let service = Arc::new(service);
    consume_capability_envelope_jti_once(service.as_ref(), &envelope)
        .await
        .context("consuming signed capability envelope replay key")?;
    best_effort_purge_capability_replay_rows(service.as_ref()).await;
    spawn_projection_retry_worker(Arc::clone(&service), reconcile_spec_folder);
    tracing::info!(
        capability_count = envelope.scope.0.len(),
        phase = %envelope.phase.as_str(),
        spec_id = %envelope.spec_id,
        tools = catalog::all_tools().len(),
        "tanren-mcp starting on stdio transport"
    );
    handler::serve_stdio(envelope.scope, service, envelope.phase).await
}

async fn consume_capability_envelope_jti_once(
    service: &tanren_app_services::methodology::MethodologyService,
    envelope: &scope::VerifiedCapabilityEnvelope,
) -> Result<()> {
    let consumed = service
        .consume_replay_guard_once(
            envelope.replay_claims.issuer.clone(),
            envelope.replay_claims.audience.clone(),
            envelope.replay_claims.jti.clone(),
            envelope.replay_claims.iat_unix,
            envelope.replay_claims.exp_unix,
        )
        .await
        .context("persisting capability-envelope replay key")?;
    if consumed {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "capability envelope replay rejected: jti has already been consumed"
        ))
    }
}

async fn best_effort_purge_capability_replay_rows(
    service: &tanren_app_services::methodology::MethodologyService,
) {
    if let Err(err) = service
        .purge_expired_replay_guards(Utc::now().timestamp(), 512)
        .await
    {
        tracing::warn!(?err, "capability-envelope replay purge tick failed");
    }
}

fn spawn_projection_retry_worker(
    service: Arc<tanren_app_services::methodology::MethodologyService>,
    spec_folder: Option<PathBuf>,
) {
    let Some(spec_folder) = spec_folder else {
        return;
    };
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(err) = service
                .reconcile_phase_events_outbox_for_folder(&spec_folder)
                .await
            {
                tracing::warn!(
                    ?err,
                    spec_folder = %spec_folder.display(),
                    "phase-event projection retry tick failed"
                );
            }
        }
    });
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("tanren_mcp=info,rmcp=warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|e| anyhow::anyhow!("tracing init: {e}"))?;
    Ok(())
}

fn writeln_stderr(msg: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    writeln!(std::io::stderr(), "{msg}")
}

struct MethodologyRuntimeSettings {
    required_guards: Vec<tanren_app_services::methodology::RequiredGuard>,
    standards_root: PathBuf,
    pillars: Vec<tanren_app_services::methodology::Pillar>,
    issue_provider: String,
}

fn load_methodology_runtime_settings(config_path: &Path) -> Result<MethodologyRuntimeSettings> {
    let default_root = resolve_relative_to_config(config_path, Path::new("tanren/standards"));
    if !config_path.exists() {
        return Ok(MethodologyRuntimeSettings {
            required_guards: vec![
                tanren_app_services::methodology::RequiredGuard::GateChecked,
                tanren_app_services::methodology::RequiredGuard::Audited,
                tanren_app_services::methodology::RequiredGuard::Adherent,
            ],
            standards_root: default_root,
            pillars: tanren_app_services::methodology::builtin_pillars(),
            issue_provider: "GitHub".to_owned(),
        });
    }
    let raw = std::fs::read_to_string(config_path)
        .with_context(|| format!("reading {}", config_path.display()))?;
    let cfg = tanren_app_services::methodology::config::TanrenConfig::from_yaml(&raw)
        .with_context(|| format!("parsing {}", config_path.display()))?;
    let standards_raw = cfg
        .methodology
        .variables
        .get("standards_root")
        .or_else(|| cfg.methodology.variables.get("STANDARDS_ROOT"))
        .map_or("tanren/standards", String::as_str);
    let pillars = tanren_app_services::methodology::rubric_registry::effective_pillars_for_runtime(
        config_path,
        &cfg,
    )
    .map_err(anyhow::Error::msg)
    .with_context(|| format!("resolving rubric pillars from {}", config_path.display()))?;
    Ok(MethodologyRuntimeSettings {
        required_guards: cfg.methodology.task_complete_requires,
        standards_root: resolve_relative_to_config(config_path, Path::new(standards_raw)),
        pillars,
        issue_provider: cfg
            .methodology
            .variables
            .get("issue_provider")
            .or_else(|| cfg.methodology.variables.get("ISSUE_PROVIDER"))
            .cloned()
            .unwrap_or_else(|| "GitHub".to_owned()),
    })
}

fn resolve_relative_to_config(config_path: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let base = config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    base.join(path)
}
