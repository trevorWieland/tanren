//! `tanren dispatch-read` and `tanren dispatch-mutation` subcommand handlers.
//!
//! The CLI exposes dispatch operations as two disjoint top-level
//! subcommand groups so mutating and reading are type-safe at compile
//! time:
//!
//! - [`DispatchReadCommand`] — read-only (`get`, `list`). No replay
//!   guard is consumed; handler signature has no `ReplayGuard`
//!   parameter at all.
//! - [`DispatchMutationCommand`] — state-changing (`create`, `cancel`).
//!   The handler requires `&ReplayGuard` by its type; the replay guard
//!   is passed by value to the store which consumes it atomically with
//!   the mutation (success) or with the policy-decision audit event
//!   (denied).
//!
//! A new mutating variant added to the enum cannot compile without
//! threading the replay guard through its handler — there is no
//! runtime `Option<&ReplayGuard>` fallback.

use std::io::Write as _;

use anyhow::Result;
use clap::Subcommand;
use tanren_app_services::{ReplayGuard, RequestContext, compose::Service};
use tanren_contract::{
    CancelDispatchRequest, CreateDispatchRequest, DispatchCursorToken, DispatchListFilter,
    ErrorResponse, parse_project_env_entries,
};
use uuid::Uuid;

/// Read-only dispatch subcommands (no replay consumption).
#[derive(Debug, Subcommand)]
pub(crate) enum DispatchReadCommand {
    /// Get a dispatch by ID.
    Get(GetArgs),
    /// List dispatches.
    List(ListArgs),
}

/// Mutating dispatch subcommands (consume replay guard exactly once).
#[derive(Debug, Subcommand)]
pub(crate) enum DispatchMutationCommand {
    /// Create a new dispatch.
    Create(Box<CreateArgs>),
    /// Cancel a dispatch.
    Cancel(CancelArgs),
}

/// Arguments for `dispatch-mutation create`.
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

/// Arguments for `dispatch-read get`.
#[derive(Debug, clap::Args)]
pub(crate) struct GetArgs {
    /// Dispatch UUID.
    #[arg(long)]
    pub id: Uuid,
}

/// Arguments for `dispatch-read list`.
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

/// Arguments for `dispatch-mutation cancel`.
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

#[cfg(test)]
mod compile_time_signature_checks {
    //! Compile-time assertions that the mutating handler requires
    //! `&ReplayGuard` and the read handler does not have a replay
    //! guard parameter. If someone refactors either handler to take
    //! `Option<&ReplayGuard>` or drop the guard parameter, the type
    //! of the extracted function pointer will change and these
    //! assertions will fail to compile.
    //!
    //! This is the lane 0.4 audit finding #4 fix at the type level:
    //! "mutating command implies replay guard consumed" is enforced
    //! by the type system, not by a runtime `Option::ok_or_else`
    //! fallback that a future refactor can silently break.

    use std::future::Future;
    use std::pin::Pin;

    use tanren_app_services::{ReplayGuard, RequestContext, compose::Service};

    use super::{DispatchMutationCommand, DispatchReadCommand};

    type ReadHandlerFn =
        for<'a> fn(
            DispatchReadCommand,
            &'a Service,
            &'a RequestContext,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

    type MutationHandlerFn =
        for<'a> fn(
            DispatchMutationCommand,
            &'a Service,
            &'a RequestContext,
            &'a ReplayGuard,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

    #[test]
    fn read_handler_signature_excludes_replay_guard() {
        let handler: ReadHandlerFn =
            |cmd, service, context| Box::pin(super::handle_read(cmd, service, context));
        // Evaluate at runtime so the type assertion is load-bearing
        // rather than dead code that inline-suppression rules flag.
        assert_eq!(size_of_val(&handler), size_of::<usize>());
    }

    #[test]
    fn mutation_handler_signature_requires_replay_guard() {
        let handler: MutationHandlerFn = |cmd, service, context, replay_guard| {
            Box::pin(super::handle_mutation(cmd, service, context, replay_guard))
        };
        assert_eq!(size_of_val(&handler), size_of::<usize>());
    }
}
