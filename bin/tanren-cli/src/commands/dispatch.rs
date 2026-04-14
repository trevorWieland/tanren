//! `tanren dispatch` subcommands — create, get, list, cancel.

use std::io::Write as _;

use anyhow::Result;
use clap::Subcommand;
use tanren_app_services::{RequestContext, compose::Service};
use tanren_contract::{
    CancelDispatchRequest, CreateDispatchRequest, DispatchCursorToken, DispatchListFilter,
    ErrorResponse, parse_project_env_entries,
};
use uuid::Uuid;

/// Dispatch management commands.
#[derive(Debug, Subcommand)]
pub(crate) enum DispatchCommand {
    /// Create a new dispatch.
    Create(Box<CreateArgs>),
    /// Get a dispatch by ID.
    Get(GetArgs),
    /// List dispatches.
    List(ListArgs),
    /// Cancel a dispatch.
    Cancel(CancelArgs),
}

impl DispatchCommand {
    /// Whether this command mutates persistent state and therefore
    /// requires migrate-before-write behavior.
    #[must_use]
    pub(crate) const fn requires_write_store(&self) -> bool {
        matches!(self, Self::Create(_) | Self::Cancel(_))
    }
}

/// Arguments for `dispatch create`.
#[derive(Debug, clap::Args)]
pub(crate) struct CreateArgs {
    /// Project name.
    #[arg(long)]
    pub project: String,
    /// Phase of work.
    #[arg(long)]
    pub phase: tanren_contract::Phase,
    /// CLI harness.
    #[arg(long)]
    pub cli: tanren_contract::Cli,
    /// Git branch.
    #[arg(long)]
    pub branch: String,
    /// Specification folder path.
    #[arg(long)]
    pub spec_folder: String,
    /// Workflow identifier.
    #[arg(long)]
    pub workflow_id: String,
    /// Dispatch mode.
    #[arg(long, default_value = "manual")]
    pub mode: tanren_contract::DispatchMode,
    /// Timeout in seconds.
    #[arg(long, default_value = "300")]
    pub timeout: u64,
    /// Environment profile.
    #[arg(long, default_value = "default")]
    pub environment_profile: String,
    /// Authentication mode.
    #[arg(long, default_value = "api_key")]
    pub auth_mode: tanren_contract::AuthMode,
    /// Gate command.
    #[arg(long)]
    pub gate_cmd: Option<String>,
    /// Context string.
    #[arg(long)]
    pub context: Option<String>,
    /// Model override.
    #[arg(long)]
    pub model: Option<String>,
    /// Non-secret environment variables, repeatable `KEY=VALUE`.
    #[arg(long = "project-env", value_name = "KEY=VALUE", action = clap::ArgAction::Append)]
    pub project_env: Vec<String>,
    /// Required runtime secret names, repeatable.
    #[arg(long = "required-secret", value_name = "SECRET_NAME", action = clap::ArgAction::Append)]
    pub required_secrets: Vec<String>,
    /// Preserve environment on failure.
    #[arg(long, default_value_t = false)]
    pub preserve_on_failure: bool,
}

/// Arguments for `dispatch get`.
#[derive(Debug, clap::Args)]
pub(crate) struct GetArgs {
    /// Dispatch UUID.
    #[arg(long)]
    pub id: Uuid,
}

/// Arguments for `dispatch list`.
#[derive(Debug, clap::Args)]
pub(crate) struct ListArgs {
    /// Filter by status.
    #[arg(long)]
    pub status: Option<tanren_contract::DispatchStatus>,
    /// Filter by lane.
    #[arg(long)]
    pub lane: Option<tanren_contract::Lane>,
    /// Filter by project.
    #[arg(long)]
    pub project: Option<String>,
    /// Maximum number of results.
    #[arg(long)]
    pub limit: Option<u64>,
    /// Opaque cursor token from previous list output.
    #[arg(long)]
    pub cursor: Option<String>,
}

/// Arguments for `dispatch cancel`.
#[derive(Debug, clap::Args)]
pub(crate) struct CancelArgs {
    /// Dispatch UUID to cancel.
    #[arg(long)]
    pub id: Uuid,
    /// Reason for cancellation.
    #[arg(long)]
    pub reason: Option<String>,
}

/// Handle a dispatch subcommand.
pub(crate) async fn handle(
    cmd: DispatchCommand,
    service: &Service,
    context: &RequestContext,
) -> Result<()> {
    match cmd {
        DispatchCommand::Create(args) => handle_create(*args, service, context).await,
        DispatchCommand::Get(args) => handle_get(args, service, context).await,
        DispatchCommand::List(args) => handle_list(args, service, context).await,
        DispatchCommand::Cancel(args) => handle_cancel(args, service, context).await,
    }
}

async fn handle_create(
    args: CreateArgs,
    service: &Service,
    context: &RequestContext,
) -> Result<()> {
    let project_env = parse_project_env_entries(args.project_env).map_err(ErrorResponse::from)?;
    let req = CreateDispatchRequest {
        project: args.project,
        phase: args.phase,
        cli: args.cli,
        branch: args.branch,
        spec_folder: args.spec_folder,
        workflow_id: args.workflow_id,
        mode: args.mode,
        timeout_secs: args.timeout,
        environment_profile: args.environment_profile,
        auth_mode: args.auth_mode,
        gate_cmd: args.gate_cmd,
        context: args.context,
        model: args.model,
        project_env,
        required_secrets: args.required_secrets,
        preserve_on_failure: args.preserve_on_failure,
    };

    let resp = service.create(context, req).await?;
    print_json(&resp)
}

async fn handle_get(args: GetArgs, service: &Service, context: &RequestContext) -> Result<()> {
    let resp = service.get(context, args.id).await?;
    print_json(&resp)
}

async fn handle_list(args: ListArgs, service: &Service, context: &RequestContext) -> Result<()> {
    let cursor = args
        .cursor
        .map(|raw| DispatchCursorToken::decode(&raw).map_err(ErrorResponse::from))
        .transpose()?;

    let filter = DispatchListFilter {
        status: args.status,
        lane: args.lane,
        project: args.project,
        limit: args.limit,
        cursor,
    };

    let resp = service.list(context, filter).await?;
    print_json(&resp)
}

async fn handle_cancel(
    args: CancelArgs,
    service: &Service,
    context: &RequestContext,
) -> Result<()> {
    let req = CancelDispatchRequest {
        dispatch_id: args.id,
        reason: args.reason,
    };

    service.cancel(context, req).await?;
    print_json(&serde_json::json!({"status": "cancelled"}))
}

/// Print a value as JSON to stdout.
fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    writeln!(std::io::stdout(), "{json}")?;
    Ok(())
}
