use std::io::Write;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Subcommand;
use tanren_app_services::{AppServiceError, Handlers, OrganizationStore, Store};
use tanren_contract::{CreateOrganizationRequest, OrganizationAdminOperation};
use tanren_identity_policy::{OrgId, OrganizationName};
use uuid::Uuid;

use crate::load_session;

#[derive(Debug, Subcommand)]
pub(crate) enum OrganizationAction {
    Create {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        name: String,
    },
    List {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
    },
    AuthorizeAdminOperation {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        org_id: String,
        #[arg(long)]
        operation: String,
    },
}

pub(crate) fn dispatch(action: OrganizationAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run(action))
}

async fn run(action: OrganizationAction) -> Result<()> {
    let handlers = Handlers::new();
    match action {
        OrganizationAction::Create { database_url, name } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let token = load_session()?;
            let org_name =
                OrganizationName::parse(&name).context("parse --name as organization name")?;
            let response = handlers
                .create_organization_with_session(
                    &store,
                    &token,
                    CreateOrganizationRequest { name: org_name },
                )
                .await
                .map_err(org_error)?;
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            let permissions: Vec<&str> = response
                .granted_permissions
                .iter()
                .map(|p| permission_label(*p))
                .collect();
            writeln!(
                handle,
                "org_id={id} name={name} project_count={count} permissions=[{perms}]",
                id = response.organization.id,
                name = response.organization.name,
                count = response.project_count,
                perms = permissions.join(", "),
            )
            .context("write create-organization result")?;
        }
        OrganizationAction::List { database_url } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let account_id = resolve_session_account(&store).await?;
            let orgs = handlers
                .list_account_organizations(&store, account_id)
                .await
                .map_err(org_error)?;
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            if orgs.is_empty() {
                writeln!(handle, "organizations: (none)").context("write list result")?;
            } else {
                for org in &orgs {
                    writeln!(
                        handle,
                        "org_id={id} name={name} project_count={count}",
                        id = org.id,
                        name = org.name,
                        count = 0u64,
                    )
                    .context("write list result")?;
                }
            }
        }
        OrganizationAction::AuthorizeAdminOperation {
            database_url,
            org_id,
            operation,
        } => {
            let store = Store::connect(&database_url)
                .await
                .context("connect to store")?;
            let account_id = resolve_session_account(&store).await?;
            let org_uuid = Uuid::parse_str(&org_id).context("parse --org-id as UUID")?;
            let org = OrgId::new(org_uuid);
            let op = parse_admin_operation(&operation)?;
            handlers
                .authorize_org_admin_operation(&store, account_id, org, op)
                .await
                .map_err(org_error)?;
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            writeln!(handle, "authorized: true").context("write authorize result")?;
        }
    }
    Ok(())
}

async fn resolve_session_account(store: &Store) -> Result<tanren_identity_policy::AccountId> {
    let token = load_session()?;
    let now = Utc::now();
    let session = store
        .resolve_bearer_session(&token, now)
        .await
        .context("resolve session")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "error: auth_required — {}",
                "Session is expired or invalid."
            )
        })?;
    Ok(session.account_id)
}

fn org_error(err: AppServiceError) -> anyhow::Error {
    match err {
        AppServiceError::Organization(reason) => {
            anyhow::anyhow!("error: {} — {}", reason.code(), reason.summary())
        }
        AppServiceError::InvalidInput(message) => {
            anyhow::anyhow!("error: validation_failed — {message}")
        }
        AppServiceError::Store(err) => {
            anyhow::anyhow!("error: internal_error — {err}")
        }
        _ => anyhow::anyhow!("error: internal_error — unknown app-service failure"),
    }
}

fn parse_admin_operation(raw: &str) -> Result<OrganizationAdminOperation> {
    match raw {
        "invite_members" => Ok(OrganizationAdminOperation::InviteMembers),
        "manage_access" => Ok(OrganizationAdminOperation::ManageAccess),
        "configure" => Ok(OrganizationAdminOperation::Configure),
        "set_policy" => Ok(OrganizationAdminOperation::SetPolicy),
        "delete" => Ok(OrganizationAdminOperation::Delete),
        _ => anyhow::bail!(
            "unknown operation '{raw}'; expected one of: \
             invite_members, manage_access, configure, set_policy, delete"
        ),
    }
}

fn permission_label(p: tanren_identity_policy::OrgPermission) -> &'static str {
    match p {
        tanren_identity_policy::OrgPermission::InviteMembers => "invite_members",
        tanren_identity_policy::OrgPermission::ManageAccess => "manage_access",
        tanren_identity_policy::OrgPermission::Configure => "configure",
        tanren_identity_policy::OrgPermission::SetPolicy => "set_policy",
        tanren_identity_policy::OrgPermission::Delete => "delete",
        _ => "unknown",
    }
}
