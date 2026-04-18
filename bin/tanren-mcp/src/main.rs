//! `tanren-mcp` — Model Context Protocol server exposing the
//! methodology tool surface over stdio.
//!
//! Lane 0.5 scope:
//!
//! - stdio transport only (rmcp `transport-io` feature).
//! - Capability scope derived from `TANREN_PHASE_CAPABILITIES` env var
//!   supplied by the orchestrator at dispatch time.
//! - Phase name from `TANREN_MCP_PHASE` env var (default `"mcp"`).
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

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;

mod catalog;
mod dispatch;
mod handler;
mod scope;

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
    let scope = scope::parse_from_env();
    let phase = std::env::var("TANREN_MCP_PHASE").unwrap_or_else(|_| "mcp".to_owned());
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
    let phase_events = std::env::var("TANREN_SPEC_FOLDER").ok().map(|spec_folder| {
        tanren_app_services::methodology::service::PhaseEventsRuntime {
            spec_folder: PathBuf::from(spec_folder),
            agent_session_id: std::env::var("TANREN_AGENT_SESSION_ID")
                .unwrap_or_else(|_| "mcp-session".to_owned()),
        }
    });
    let service = tanren_app_services::compose::build_methodology_service_with_config(
        &database_url,
        runtime.required_guards,
        phase_events,
        standards,
    )
    .await
    .context("building methodology service")?;
    tracing::info!(
        capability_count = scope.0.len(),
        phase = %phase,
        tools = catalog::all_tools().len(),
        "tanren-mcp starting on stdio transport"
    );
    handler::serve_stdio(scope, Arc::new(service), phase).await
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
    Ok(MethodologyRuntimeSettings {
        required_guards: cfg.methodology.task_complete_requires,
        standards_root: resolve_relative_to_config(config_path, Path::new(standards_raw)),
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
