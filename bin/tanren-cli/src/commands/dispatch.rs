//! `tanren dispatch` subcommand handlers.
//!
//! The public CLI surface is `tanren dispatch {create|get|list|cancel}`.
//! Internally, that flat parser is projected into two **type-distinct** request
//! kinds before any handler runs:
//!
//! - [`DispatchReadCommand`] — read-only (`get`, `list`). No replay
//!   guard is consumed; the handler signature has no `ReplayGuard`
//!   parameter at all.
//! - [`DispatchMutationCommand`] — state-changing (`create`, `cancel`).
//!   The handler requires `&ReplayGuard` by its type; the replay guard
//!   is passed by value to the store which consumes it atomically with
//!   the mutation (success) or with the policy-decision audit event
//!   (denied).
//!
//! A new mutating variant added to the mutation enum cannot compile
//! without threading the replay guard through its handler — there is
//! no runtime `Option<&ReplayGuard>` fallback.
//!
//! The external `dispatch-read` / `dispatch-mutation` verbs that
//! briefly leaked this internal split are **not** exposed. Callers
//! type `tanren dispatch <verb>`; the split is an internal wiring
//! detail, preserved at the type level inside
//! [`DispatchCommand::split`].

use std::io::Write as _;

use anyhow::Result;
use clap::Subcommand;
use tanren_app_services::{ReplayGuard, RequestContext, compose::Service};
use tanren_contract::{
    CancelDispatchRequest, CreateDispatchRequest, DispatchCursorToken, DispatchListFilter,
    ErrorResponse, parse_project_env_entries,
};
use uuid::Uuid;

use super::enums::{AuthModeArg, CliArg, DispatchModeArg, DispatchStatusArg, LaneArg, PhaseArg};

/// Public `tanren dispatch <verb>` parser.
///
/// This is the external CLI surface. Internally it projects to
/// [`DispatchRequest`] which is the type-safe wiring boundary that
/// separates read from mutation.
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

/// Type-distinct projection of a parsed [`DispatchCommand`].
///
/// `Read` variants never carry a replay guard obligation; `Mutation`
/// variants must thread one. The compile-time signature checks in
/// the test module below lock this invariant in place.
pub(crate) enum DispatchRequest {
    Read(DispatchReadCommand),
    Mutation(DispatchMutationCommand),
}

impl DispatchCommand {
    /// Project the flat public command into a type-safe read/mutation
    /// request.
    pub(crate) fn split(self) -> DispatchRequest {
        match self {
            Self::Create(args) => DispatchRequest::Mutation(DispatchMutationCommand::Create(args)),
            Self::Get(args) => DispatchRequest::Read(DispatchReadCommand::Get(args)),
            Self::List(args) => DispatchRequest::Read(DispatchReadCommand::List(args)),
            Self::Cancel(args) => DispatchRequest::Mutation(DispatchMutationCommand::Cancel(args)),
        }
    }
}

/// Read-only dispatch subcommands (no replay consumption).
pub(crate) enum DispatchReadCommand {
    Get(GetArgs),
    List(ListArgs),
}

/// Mutating dispatch subcommands (consume replay guard exactly once).
pub(crate) enum DispatchMutationCommand {
    Create(Box<CreateArgs>),
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
    pub phase: PhaseArg,
    /// CLI harness.
    #[arg(long)]
    pub cli: CliArg,
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
    pub mode: DispatchModeArg,
    /// Timeout in seconds.
    #[arg(long, default_value = "300")]
    pub timeout: u64,
    /// Environment profile.
    #[arg(long, default_value = "default")]
    pub environment_profile: String,
    /// Authentication mode.
    #[arg(long, default_value = "api_key")]
    pub auth_mode: AuthModeArg,
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
    pub status: Option<DispatchStatusArg>,
    /// Filter by lane.
    #[arg(long)]
    pub lane: Option<LaneArg>,
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

/// Handle a read-only dispatch subcommand.
///
/// The signature intentionally has no replay guard parameter — read
/// commands cannot consume replay state.
pub(crate) async fn handle_read(
    cmd: DispatchReadCommand,
    service: &Service,
    context: &RequestContext,
) -> Result<()> {
    match cmd {
        DispatchReadCommand::Get(args) => handle_get(args, service, context).await,
        DispatchReadCommand::List(args) => handle_list(args, service, context).await,
    }
}

/// Handle a mutating dispatch subcommand.
///
/// The `&ReplayGuard` parameter is not `Option`: every mutating command
/// requires the caller to thread a verified replay guard from the
/// signed actor token. The store consumes the guard atomically.
pub(crate) async fn handle_mutation(
    cmd: DispatchMutationCommand,
    service: &Service,
    context: &RequestContext,
    replay_guard: &ReplayGuard,
) -> Result<()> {
    match cmd {
        DispatchMutationCommand::Create(args) => {
            handle_create(*args, service, context, replay_guard).await
        }
        DispatchMutationCommand::Cancel(args) => {
            handle_cancel(args, service, context, replay_guard).await
        }
    }
}

async fn handle_create(
    args: CreateArgs,
    service: &Service,
    context: &RequestContext,
    replay_guard: &ReplayGuard,
) -> Result<()> {
    let project_env = parse_project_env_entries(args.project_env).map_err(ErrorResponse::from)?;
    let req = CreateDispatchRequest {
        project: args.project,
        phase: args.phase.into(),
        cli: args.cli.into(),
        branch: args.branch,
        spec_folder: args.spec_folder,
        workflow_id: args.workflow_id,
        mode: args.mode.into(),
        timeout_secs: args.timeout,
        environment_profile: args.environment_profile,
        auth_mode: args.auth_mode.into(),
        gate_cmd: args.gate_cmd,
        context: args.context,
        model: args.model,
        project_env,
        required_secrets: args.required_secrets,
        preserve_on_failure: args.preserve_on_failure,
    };

    let resp = service.create(context, req, replay_guard).await?;
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
        status: args.status.map(Into::into),
        lane: args.lane.map(Into::into),
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
    replay_guard: &ReplayGuard,
) -> Result<()> {
    let req = CancelDispatchRequest {
        dispatch_id: args.id,
        reason: args.reason,
    };

    service.cancel(context, req, replay_guard).await?;
    print_json(&serde_json::json!({"status": "cancelled"}))
}

/// Print a value as JSON to stdout.
fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    writeln!(std::io::stdout(), "{json}")?;
    Ok(())
}
