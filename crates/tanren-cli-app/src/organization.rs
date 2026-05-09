use std::fs;
use std::io::Write;

use anyhow::{Context, Result};
use clap::Subcommand;
use secrecy::SecretString;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{
    AccountFailureReason, CheckOrganizationPermissionRequest, CreateOrganizationRequest,
    ListOrganizationsRequest,
};
use tanren_identity_policy::{
    AccountId, OrgId, OrganizationName, OrganizationPermission, SessionToken,
};
use uuid::Uuid;

use crate::{account_error, session_path};

#[derive(Debug, Subcommand)]
pub(crate) enum OrganizationAction {
    /// Create a new organization for the signed-in account.
    Create {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Signed-in account id.
        #[arg(long)]
        account_id: String,
        /// Organization name.
        #[arg(long)]
        name: String,
    },
    /// List organizations available to the signed-in account.
    List {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Signed-in account id.
        #[arg(long)]
        account_id: String,
    },
    /// Check an admin permission inside an organization.
    CheckPermission {
        /// Database URL.
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        /// Signed-in account id.
        #[arg(long)]
        account_id: String,
        /// Organization id.
        #[arg(long)]
        org_id: String,
        /// Permission key: `invite|manage_access|configure|set_policy|delete`.
        #[arg(long)]
        permission: String,
    },
}

pub(crate) async fn run_organization(
    action: OrganizationAction,
    handlers: &Handlers,
) -> Result<()> {
    match action {
        OrganizationAction::Create {
            database_url,
            account_id,
            name,
        } => create_organization(handlers, &database_url, &account_id, &name).await?,
        OrganizationAction::List {
            database_url,
            account_id,
        } => list_organizations(handlers, &database_url, &account_id).await?,
        OrganizationAction::CheckPermission {
            database_url,
            account_id,
            org_id,
            permission,
        } => check_permission(handlers, &database_url, &account_id, &org_id, &permission).await?,
    }
    Ok(())
}

async fn create_organization(
    handlers: &Handlers,
    database_url: &str,
    account_id_raw: &str,
    name_raw: &str,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let account_id = parse_account_id(account_id_raw)?;
    let session_token = read_session_token()?;
    let name = OrganizationName::parse(name_raw).context("parse --name as organization name")?;
    let response = handlers
        .create_organization(
            &store,
            CreateOrganizationRequest {
                session_token,
                account_id,
                name,
                idempotency_key: None,
            },
        )
        .await
        .map_err(account_error)?;
    let granted = response
        .granted_permissions
        .iter()
        .map(|p| permission_key(*p))
        .collect::<Vec<_>>()
        .join(",");
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "organization_id={id} name={name} granted_permissions={granted}",
        id = response.organization.id,
        name = response.organization.name,
        granted = granted,
    )
    .context("write create-organization result")?;
    Ok(())
}

async fn list_organizations(
    handlers: &Handlers,
    database_url: &str,
    account_id_raw: &str,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let account_id = parse_account_id(account_id_raw)?;
    let session_token = read_session_token()?;
    let response = handlers
        .list_organizations(
            &store,
            ListOrganizationsRequest {
                session_token,
                account_id,
            },
        )
        .await
        .map_err(account_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    if response.organizations.is_empty() {
        writeln!(handle, "organizations=0").context("write organization-list result")?;
    } else {
        writeln!(handle, "organizations={}", response.organizations.len())
            .context("write organization-list count")?;
        for org in response.organizations {
            writeln!(handle, "organization_id={} name={}", org.id, org.name)
                .context("write organization-list row")?;
        }
    }
    Ok(())
}

async fn check_permission(
    handlers: &Handlers,
    database_url: &str,
    account_id_raw: &str,
    org_id_raw: &str,
    permission_raw: &str,
) -> Result<()> {
    let store = Store::connect(database_url)
        .await
        .context("connect to store")?;
    let account_id = parse_account_id(account_id_raw)?;
    let org_id = parse_org_id(org_id_raw)?;
    let permission = parse_permission(permission_raw)?;
    let session_token = read_session_token()?;
    let response = handlers
        .check_organization_permission(
            &store,
            CheckOrganizationPermissionRequest {
                session_token,
                account_id,
                org_id,
                permission,
            },
        )
        .await
        .map_err(account_error)?;
    if !response.allowed {
        return Err(account_error(AppServiceError::Account(
            AccountFailureReason::PermissionDenied,
        )));
    }
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "account_id={account} org_id={org} permission={permission} allowed=true",
        account = response.account_id,
        org = response.org_id,
        permission = permission_key(response.permission),
    )
    .context("write permission-check result")?;
    Ok(())
}

fn read_session_token() -> Result<SessionToken> {
    let path = session_path();
    let token = match fs::read_to_string(&path) {
        Ok(token) => token,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(account_error(AppServiceError::Account(
                AccountFailureReason::AuthRequired,
            )));
        }
        Err(err) => {
            return Err(err).with_context(|| format!("read session from {}", path.display()));
        }
    };
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(account_error(AppServiceError::Account(
            AccountFailureReason::AuthRequired,
        )));
    }
    Ok(SessionToken::from_secret(SecretString::from(
        trimmed.to_owned(),
    )))
}

fn parse_account_id(raw: &str) -> Result<AccountId> {
    let uuid = Uuid::parse_str(raw).context("parse --account-id as uuid")?;
    Ok(AccountId::from(uuid))
}

fn parse_org_id(raw: &str) -> Result<OrgId> {
    let uuid = Uuid::parse_str(raw).context("parse --org-id as uuid")?;
    Ok(OrgId::from(uuid))
}

fn parse_permission(raw: &str) -> Result<OrganizationPermission> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "invite" => Ok(OrganizationPermission::Invite),
        "manage_access" => Ok(OrganizationPermission::ManageAccess),
        "configure" => Ok(OrganizationPermission::Configure),
        "set_policy" => Ok(OrganizationPermission::SetPolicy),
        "delete" => Ok(OrganizationPermission::Delete),
        _ => Err(anyhow::anyhow!(
            "parse --permission: expected invite|manage_access|configure|set_policy|delete"
        )),
    }
}

fn permission_key(permission: OrganizationPermission) -> &'static str {
    match permission {
        OrganizationPermission::Invite => "invite",
        OrganizationPermission::ManageAccess => "manage_access",
        OrganizationPermission::Configure => "configure",
        OrganizationPermission::SetPolicy => "set_policy",
        OrganizationPermission::Delete => "delete",
    }
}
