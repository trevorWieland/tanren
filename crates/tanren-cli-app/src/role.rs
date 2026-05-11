use std::io::Write;

use crate::read_authenticated_actor;
use anyhow::{Context, Result};
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

impl ScopeKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::Organization => "organization",
            Self::Project => "project",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum PrincipalKind {
    Account,
    Role,
}

impl PrincipalKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::Role => "role",
        }
    }
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
    ParsedRoleAction::from(action).run().await
}

enum ParsedRoleAction {
    Create(CreateArgs),
    Edit(EditArgs),
    Delete(DeleteArgs),
    Apply(ApplyArgs),
    Check(CheckArgs),
}

impl ParsedRoleAction {
    fn from(action: RoleAction) -> Self {
        match action {
            RoleAction::Create {
                database_url,
                scope_kind,
                scope_id,
                name,
                permissions,
            } => Self::Create(CreateArgs {
                database_url,
                scope_kind,
                scope_id,
                name,
                permissions,
            }),
            RoleAction::Edit {
                database_url,
                role_id,
                scope_kind,
                scope_id,
                name,
                permissions,
            } => Self::Edit(EditArgs {
                database_url,
                role_id,
                scope_kind,
                scope_id,
                name,
                permissions,
            }),
            RoleAction::Delete {
                database_url,
                role_id,
                scope_kind,
                scope_id,
            } => Self::Delete(DeleteArgs {
                database_url,
                role_id,
                scope_kind,
                scope_id,
            }),
            RoleAction::Apply {
                database_url,
                role_id,
                role_scope_kind,
                role_scope_id,
                principal_kind,
                principal_id,
                grant_scope_kind,
                grant_scope_id,
            } => Self::Apply(ApplyArgs {
                database_url,
                role_id,
                role_scope_kind,
                role_scope_id,
                principal_kind,
                principal_id,
                grant_scope_kind,
                grant_scope_id,
            }),
            RoleAction::Check {
                database_url,
                principal_kind,
                principal_id,
                permission,
                scope_kind,
                scope_id,
            } => Self::Check(CheckArgs {
                database_url,
                principal_kind,
                principal_id,
                permission,
                scope_kind,
                scope_id,
            }),
        }
    }

    async fn run(self) -> Result<()> {
        match self {
            Self::Create(args) => run_create(args).await,
            Self::Edit(args) => run_edit(args).await,
            Self::Delete(args) => run_delete(args).await,
            Self::Apply(args) => run_apply(args).await,
            Self::Check(args) => run_check(args).await,
        }
    }
}

struct CreateArgs {
    database_url: String,
    scope_kind: ScopeKind,
    scope_id: String,
    name: String,
    permissions: Vec<String>,
}

async fn run_create(args: CreateArgs) -> Result<()> {
    let CreateArgs {
        database_url,
        scope_kind,
        scope_id,
        name,
        permissions,
    } = args;
    let store = connect_store(&database_url).await?;
    let request = CreateRoleRequest {
        scope: parse_role_scope(scope_kind, &scope_id, "--scope-id")?,
        name: RoleName::parse(&name).context("parse --name as role name")?,
        permissions: parse_permissions(permissions)?,
    };
    let actor = authenticated_role_actor()?;
    let response = Handlers::new()
        .create_role(&store, actor, request)
        .await
        .map_err(role_error)?;
    write_json_response(&response).context("write create-role result")
}

struct EditArgs {
    database_url: String,
    role_id: String,
    scope_kind: ScopeKind,
    scope_id: String,
    name: String,
    permissions: Vec<String>,
}

async fn run_edit(args: EditArgs) -> Result<()> {
    let EditArgs {
        database_url,
        role_id,
        scope_kind,
        scope_id,
        name,
        permissions,
    } = args;
    let store = connect_store(&database_url).await?;
    let request = EditRoleRequest {
        role: parse_scoped_role(&role_id, scope_kind, &scope_id, "--scope-id")?,
        name: RoleName::parse(&name).context("parse --name as role name")?,
        permissions: parse_permissions(permissions)?,
    };
    let actor = authenticated_role_actor()?;
    let response = Handlers::new()
        .edit_role(&store, actor, request)
        .await
        .map_err(role_error)?;
    write_json_response(&response).context("write edit-role result")
}

struct DeleteArgs {
    database_url: String,
    role_id: String,
    scope_kind: ScopeKind,
    scope_id: String,
}

async fn run_delete(args: DeleteArgs) -> Result<()> {
    let DeleteArgs {
        database_url,
        role_id,
        scope_kind,
        scope_id,
    } = args;
    let store = connect_store(&database_url).await?;
    let request = DeleteRoleRequest {
        role: parse_scoped_role(&role_id, scope_kind, &scope_id, "--scope-id")?,
    };
    let actor = authenticated_role_actor()?;
    let response = Handlers::new()
        .delete_role(&store, actor, request)
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
    let store = connect_store(&args.database_url).await?;
    let request = ApplyRoleRequest {
        role: parse_scoped_role(
            &args.role_id,
            args.role_scope_kind,
            &args.role_scope_id,
            "--role-scope-id",
        )?,
        principal: parse_principal(args.principal_kind, &args.principal_id)?,
        grant_scope: parse_permission_scope(
            args.grant_scope_kind,
            &args.grant_scope_id,
            "--grant-scope-id",
        )?,
    };
    let actor = authenticated_role_actor()?;
    let response = Handlers::new()
        .apply_role(&store, actor, request)
        .await
        .map_err(role_error)?;
    write_json_response(&response).context("write apply-role result")
}

struct CheckArgs {
    database_url: String,
    principal_kind: PrincipalKind,
    principal_id: String,
    permission: String,
    scope_kind: ScopeKind,
    scope_id: String,
}

async fn run_check(args: CheckArgs) -> Result<()> {
    let CheckArgs {
        database_url,
        principal_kind,
        principal_id,
        permission,
        scope_kind,
        scope_id,
    } = args;
    let store = connect_store(&database_url).await?;
    let permission =
        PermissionName::parse(&permission).context("parse --permission as permission")?;
    let request = PermissionCheckRequest {
        principal: parse_principal(principal_kind, &principal_id)?,
        permission,
        scope: parse_permission_scope(scope_kind, &scope_id, "--scope-id")?,
    };
    let actor = authenticated_role_actor()?;
    let response = Handlers::new()
        .check_permission(&store, actor, request)
        .await
        .map_err(role_error)?;
    write_json_response(&response).context("write permission-check result")
}

async fn connect_store(database_url: &str) -> Result<Store> {
    Store::connect(database_url)
        .await
        .context("connect to store")
}

fn parse_scoped_role(
    role_id: &str,
    scope_kind: ScopeKind,
    scope_id: &str,
    scope_id_field: &'static str,
) -> Result<ScopedRole> {
    Ok(ScopedRole {
        role_id: parse_role_id(role_id)?,
        scope: parse_role_scope(scope_kind, scope_id, scope_id_field)?,
    })
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
            tracing::error!(target: "tanren_cli", error = %err, "role store failure");
            anyhow::anyhow!("error: internal_error — Tanren encountered an internal error.")
        }
        other => {
            tracing::error!(target: "tanren_cli", error = ?other, "unexpected role failure");
            anyhow::anyhow!("error: internal_error — Tanren encountered an internal error.")
        }
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
