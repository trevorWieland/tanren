//! `tanren dispatch` subcommands — create, get, list, cancel.

use std::io::Write as _;

use anyhow::Result;
use clap::Subcommand;
use tanren_app_services::compose::Service;
use tanren_contract::{CreateDispatchRequest, DispatchListFilter, ErrorResponse};
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

/// Arguments for `dispatch create`.
#[derive(Debug, clap::Args)]
pub(crate) struct CreateArgs {
    /// Project name.
    #[arg(long)]
    pub project: String,
    /// Phase of work.
    #[arg(long)]
    pub phase: String,
    /// CLI harness.
    #[arg(long)]
    pub cli: String,
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
    pub mode: String,
    /// Timeout in seconds.
    #[arg(long, default_value = "300")]
    pub timeout: u64,
    /// Environment profile.
    #[arg(long, default_value = "default")]
    pub environment_profile: String,
    /// Organization UUID.
    #[arg(long)]
    pub org_id: Uuid,
    /// User UUID.
    #[arg(long)]
    pub user_id: Uuid,
    /// Authentication mode.
    #[arg(long)]
    pub auth_mode: Option<String>,
    /// Gate command.
    #[arg(long)]
    pub gate_cmd: Option<String>,
    /// Context string.
    #[arg(long)]
    pub context: Option<String>,
    /// Model override.
    #[arg(long)]
    pub model: Option<String>,
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
    pub status: Option<String>,
    /// Filter by lane.
    #[arg(long)]
    pub lane: Option<String>,
    /// Filter by project.
    #[arg(long)]
    pub project: Option<String>,
    /// Maximum number of results.
    #[arg(long)]
    pub limit: Option<u64>,
    /// Number of results to skip.
    #[arg(long)]
    pub offset: Option<u64>,
}

/// Arguments for `dispatch cancel`.
#[derive(Debug, clap::Args)]
pub(crate) struct CancelArgs {
    /// Dispatch UUID to cancel.
    #[arg(long)]
    pub id: Uuid,
    /// Organization UUID.
    #[arg(long)]
    pub org_id: Uuid,
    /// User UUID.
    #[arg(long)]
    pub user_id: Uuid,
    /// Reason for cancellation.
    #[arg(long)]
    pub reason: Option<String>,
}

/// Handle a dispatch subcommand.
pub(crate) async fn handle(cmd: DispatchCommand, service: &Service) -> Result<()> {
    match cmd {
        DispatchCommand::Create(args) => handle_create(*args, service).await,
        DispatchCommand::Get(args) => handle_get(args, service).await,
        DispatchCommand::List(args) => handle_list(args, service).await,
        DispatchCommand::Cancel(args) => handle_cancel(args, service).await,
    }
}

async fn handle_create(args: CreateArgs, service: &Service) -> Result<()> {
    let req = CreateDispatchRequest {
        org_id: args.org_id,
        user_id: args.user_id,
        project: args.project,
        phase: parse_enum("phase", &args.phase)?,
        cli: parse_enum("cli", &args.cli)?,
        branch: args.branch,
        spec_folder: args.spec_folder,
        workflow_id: args.workflow_id,
        mode: parse_enum("mode", &args.mode)?,
        timeout_secs: args.timeout,
        environment_profile: args.environment_profile,
        team_id: None,
        api_key_id: None,
        project_id: None,
        auth_mode: args
            .auth_mode
            .as_deref()
            .map(|s| parse_enum("auth_mode", s))
            .transpose()?,
        gate_cmd: args.gate_cmd,
        context: args.context,
        model: args.model,
        project_env: None,
        required_secrets: None,
        preserve_on_failure: None,
    };

    let resp = service.create(req).await?;
    print_json(&resp)
}

async fn handle_get(args: GetArgs, service: &Service) -> Result<()> {
    let resp = service.get(args.id).await?;
    print_json(&resp)
}

async fn handle_list(args: ListArgs, service: &Service) -> Result<()> {
    let filter = DispatchListFilter {
        status: args
            .status
            .as_deref()
            .map(|s| parse_enum("status", s))
            .transpose()?,
        lane: args
            .lane
            .as_deref()
            .map(|s| parse_enum("lane", s))
            .transpose()?,
        project: args.project,
        limit: args.limit,
        offset: args.offset,
    };

    let resp = service.list(filter).await?;
    print_json(&resp)
}

async fn handle_cancel(args: CancelArgs, service: &Service) -> Result<()> {
    let req = tanren_contract::CancelDispatchRequest {
        dispatch_id: args.id,
        org_id: args.org_id,
        user_id: args.user_id,
        team_id: None,
        reason: args.reason,
    };

    service.cancel(req).await?;
    print_json(&serde_json::json!({"status": "cancelled"}))
}

/// Print a value as JSON to stdout.
fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    writeln!(std::io::stdout(), "{json}")?;
    Ok(())
}

/// Parse a CLI string arg into a serde-deserializable domain enum.
fn parse_enum<T>(field: &str, value: &str) -> Result<T, ErrorResponse>
where
    T: serde::de::DeserializeOwned,
{
    let quoted = format!("\"{value}\"");
    serde_json::from_str::<T>(&quoted).map_err(|_| {
        ErrorResponse::from(tanren_contract::ContractError::InvalidField {
            field: field.to_owned(),
            reason: format!("unknown value: {value:?}"),
        })
    })
}
