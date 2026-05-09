use std::io::Write;

use crate::read_authenticated_actor;
use anyhow::{Context, Result, anyhow};
use clap::{Subcommand, ValueEnum};
use tanren_app_services::{Handlers, RoleServiceError, Store};
use tanren_contract::{
    ApplyRoleRequest, CreateRoleRequest, DeleteRoleRequest, EditRoleRequest,
    PermissionCheckRequest, RoleActor,
};
use tanren_identity_policy::{PermissionName, RoleName, ScopedRole};
#[path = "role_parse.rs"]
mod parse;
use parse::{
    parse_permission_scope, parse_permissions, parse_principal, parse_role_id, parse_role_scope,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ScopeKind {
    Account,
    Organization,
    Project,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum PrincipalKind {
    Account,
    Role,
}

#[derive(Debug, Subcommand)]
pub(crate) enum RoleAction {
    /// Create a role template.
    Create {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Role scope kind.
        #[arg(long, value_enum)]
        scope_kind: ScopeKind,
        /// Role scope id (UUID).
        #[arg(long)]
        scope_id: String,
        /// Role display name.
        #[arg(long)]
        name: String,
        /// Permission names bundled by this role.
        #[arg(long = "permission", required = true)]
        permissions: Vec<String>,
    },
    /// Edit an existing role template.
    Edit {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Role template id (UUID).
        #[arg(long)]
        role_id: String,
        /// Role scope kind.
        #[arg(long, value_enum)]
        scope_kind: ScopeKind,
        /// Role scope id (UUID).
        #[arg(long)]
        scope_id: String,
        /// Replacement role display name.
        #[arg(long)]
        name: String,
        /// Replacement permission bundle.
        #[arg(long = "permission", required = true)]
        permissions: Vec<String>,
    },
    /// Delete an existing role template.
    Delete {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Role template id (UUID).
        #[arg(long)]
        role_id: String,
        /// Role scope kind.
        #[arg(long, value_enum)]
        scope_kind: ScopeKind,
        /// Role scope id (UUID).
        #[arg(long)]
        scope_id: String,
    },
    /// Apply a role template to a principal.
    Apply {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Role template id (UUID).
        #[arg(long)]
        role_id: String,
        /// Role scope kind.
        #[arg(long, value_enum)]
        role_scope_kind: ScopeKind,
        /// Role scope id (UUID).
        #[arg(long)]
        role_scope_id: String,
        /// Principal kind.
        #[arg(long, value_enum)]
        principal_kind: PrincipalKind,
        /// Principal id (UUID).
        #[arg(long)]
        principal_id: String,
        /// Grant scope kind.
        #[arg(long, value_enum)]
        grant_scope_kind: ScopeKind,
        /// Grant scope id (UUID).
        #[arg(long)]
        grant_scope_id: String,
    },
    /// Check whether a principal has a permission.
    Check {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Principal kind.
        #[arg(long, value_enum)]
        principal_kind: PrincipalKind,
        /// Principal id (UUID).
        #[arg(long)]
        principal_id: String,
        /// Permission name.
        #[arg(long)]
        permission: String,
        /// Permission scope kind.
        #[arg(long, value_enum)]
        scope_kind: ScopeKind,
        /// Permission scope id (UUID).
        #[arg(long)]
        scope_id: String,
    },
}

pub(crate) fn dispatch(action: RoleAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run_role(action))
}

async fn run_role(action: RoleAction) -> Result<()> {
    match action {
        RoleAction::Create { .. } => run_create_from_action(action).await,
        RoleAction::Edit { .. } => run_edit_from_action(action).await,
        RoleAction::Delete { .. } => run_delete_from_action(action).await,
        RoleAction::Apply { .. } => run_apply_from_action(action).await,
        RoleAction::Check { .. } => run_check_from_action(action).await,
    }
}

async fn run_create_from_action(action: RoleAction) -> Result<()> {
    if let RoleAction::Create {
        database_url,
        scope_kind,
        scope_id,
        name,
        permissions,
    } = action
    {
        return run_create(database_url, scope_kind, scope_id, name, permissions).await;
    }
    Err(anyhow!("internal role action dispatch mismatch"))
}

async fn run_edit_from_action(action: RoleAction) -> Result<()> {
    if let RoleAction::Edit {
        database_url,
        role_id,
        scope_kind,
        scope_id,
        name,
        permissions,
    } = action
    {
        return run_edit(
            database_url,
            role_id,
            scope_kind,
            scope_id,
            name,
            permissions,
        )
        .await;
    }
    Err(anyhow!("internal role action dispatch mismatch"))
}

async fn run_delete_from_action(action: RoleAction) -> Result<()> {
    if let RoleAction::Delete {
        database_url,
        role_id,
        scope_kind,
        scope_id,
    } = action
    {
        return run_delete(database_url, role_id, scope_kind, scope_id).await;
    }
    Err(anyhow!("internal role action dispatch mismatch"))
}

async fn run_apply_from_action(action: RoleAction) -> Result<()> {
    if let RoleAction::Apply {
        database_url,
        role_id,
        role_scope_kind,
        role_scope_id,
        principal_kind,
        principal_id,
        grant_scope_kind,
        grant_scope_id,
    } = action
    {
        return run_apply(ApplyArgs {
            database_url,
            role_id,
            role_scope_kind,
            role_scope_id,
            principal_kind,
            principal_id,
            grant_scope_kind,
            grant_scope_id,
        })
        .await;
    }
    Err(anyhow!("internal role action dispatch mismatch"))
}

async fn run_check_from_action(action: RoleAction) -> Result<()> {
    if let RoleAction::Check {
        database_url,
        principal_kind,
        principal_id,
        permission,
        scope_kind,
        scope_id,
    } = action
    {
        return run_check(
            database_url,
            principal_kind,
            principal_id,
            permission,
            scope_kind,
            scope_id,
        )
        .await;
    }
    Err(anyhow!("internal role action dispatch mismatch"))
}

async fn run_create(
    database_url: String,
    scope_kind: ScopeKind,
    scope_id: String,
    name: String,
    permissions: Vec<String>,
) -> Result<()> {
    let store = Store::connect(&database_url)
        .await
        .context("connect to store")?;
    let scope = parse_role_scope(scope_kind, &scope_id, "--scope-id")?;
    let name = RoleName::parse(&name).context("parse --name as role name")?;
    let permissions = parse_permissions(permissions)?;
    let actor = authenticated_role_actor()?;
    let response = Handlers::new()
        .create_role(
            &store,
            actor,
            CreateRoleRequest {
                scope,
                name,
                permissions,
            },
        )
        .await
        .map_err(role_error)?;
    write_json_response(&response).context("write create-role result")
}

async fn run_edit(
    database_url: String,
    role_id: String,
    scope_kind: ScopeKind,
    scope_id: String,
    name: String,
    permissions: Vec<String>,
) -> Result<()> {
    let store = Store::connect(&database_url)
        .await
        .context("connect to store")?;
    let role = ScopedRole {
        role_id: parse_role_id(&role_id)?,
        scope: parse_role_scope(scope_kind, &scope_id, "--scope-id")?,
    };
    let name = RoleName::parse(&name).context("parse --name as role name")?;
    let permissions = parse_permissions(permissions)?;
    let actor = authenticated_role_actor()?;
    let response = Handlers::new()
        .edit_role(
            &store,
            actor,
            EditRoleRequest {
                role,
                name,
                permissions,
            },
        )
        .await
        .map_err(role_error)?;
    write_json_response(&response).context("write edit-role result")
}

async fn run_delete(
    database_url: String,
    role_id: String,
    scope_kind: ScopeKind,
    scope_id: String,
) -> Result<()> {
    let store = Store::connect(&database_url)
        .await
        .context("connect to store")?;
    let actor = authenticated_role_actor()?;
    let response = Handlers::new()
        .delete_role(
            &store,
            actor,
            DeleteRoleRequest {
                role: ScopedRole {
                    role_id: parse_role_id(&role_id)?,
                    scope: parse_role_scope(scope_kind, &scope_id, "--scope-id")?,
                },
            },
        )
        .await
        .map_err(role_error)?;
    write_json_response(&response).context("write delete-role result")
}

struct ApplyArgs {
    database_url: String,
    role_id: String,
    role_scope_kind: ScopeKind,
    role_scope_id: String,
    principal_kind: PrincipalKind,
    principal_id: String,
    grant_scope_kind: ScopeKind,
    grant_scope_id: String,
}

async fn run_apply(args: ApplyArgs) -> Result<()> {
    let store = Store::connect(&args.database_url)
        .await
        .context("connect to store")?;
    let actor = authenticated_role_actor()?;
    let response = Handlers::new()
        .apply_role(
            &store,
            actor,
            ApplyRoleRequest {
                role: ScopedRole {
                    role_id: parse_role_id(&args.role_id)?,
                    scope: parse_role_scope(
                        args.role_scope_kind,
                        &args.role_scope_id,
                        "--role-scope-id",
                    )?,
                },
                principal: parse_principal(args.principal_kind, &args.principal_id)?,
                grant_scope: parse_permission_scope(
                    args.grant_scope_kind,
                    &args.grant_scope_id,
                    "--grant-scope-id",
                )?,
            },
        )
        .await
        .map_err(role_error)?;
    write_json_response(&response).context("write apply-role result")
}

async fn run_check(
    database_url: String,
    principal_kind: PrincipalKind,
    principal_id: String,
    permission: String,
    scope_kind: ScopeKind,
    scope_id: String,
) -> Result<()> {
    let store = Store::connect(&database_url)
        .await
        .context("connect to store")?;
    let permission =
        PermissionName::parse(&permission).context("parse --permission as permission")?;
    let actor = authenticated_role_actor()?;
    let response = Handlers::new()
        .check_permission(
            &store,
            actor,
            PermissionCheckRequest {
                principal: parse_principal(principal_kind, &principal_id)?,
                permission,
                scope: parse_permission_scope(scope_kind, &scope_id, "--scope-id")?,
            },
        )
        .await
        .map_err(role_error)?;
    write_json_response(&response).context("write permission-check result")
}

fn role_error(err: RoleServiceError) -> anyhow::Error {
    match err {
        RoleServiceError::Role(reason) => {
            anyhow::anyhow!("error: {} — {}", reason.code(), reason.summary())
        }
        RoleServiceError::InvalidInput(message) => {
            anyhow::anyhow!("error: validation_failed — {message}")
        }
        RoleServiceError::Store(err) => {
            anyhow::anyhow!("error: internal_error — {err}")
        }
        _ => anyhow::anyhow!("error: internal_error — unknown app-service failure"),
    }
}

fn write_json_response<T>(response: &T) -> Result<()>
where
    T: serde::Serialize,
{
    let encoded = serde_json::to_string(response).context("encode response as json")?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{encoded}").context("write response json")
}

fn authenticated_role_actor() -> Result<RoleActor> {
    Ok(RoleActor {
        account_id: read_authenticated_actor().context("load authenticated actor from session")?,
    })
}
