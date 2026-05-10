use std::fs;
use std::io::Write;
use std::str::FromStr;

use clap::Subcommand;
use secrecy::SecretString;
use tanren_app_services::{AppServiceError, Handlers, Store, map_organization_error};
use tanren_contract::{
    AccountFailureReason, CheckOrganizationPermissionRequest, CreateOrganizationRequest,
    LIST_ORGANIZATIONS_DEFAULT_LIMIT, ListOrganizationsRequest,
};
use tanren_identity_policy::{
    AccountId, MembershipId, OrgId, OrganizationName, OrganizationPermission, SessionToken,
};
use thiserror::Error;
use uuid::Uuid;

use crate::session_path;

type OrganizationResult<T> = Result<T, OrganizationCliError>;

#[derive(Debug, Error)]
pub(crate) enum OrganizationCliError {
    #[error("error: {code} — {summary}")]
    Taxonomy { code: String, summary: String },
    #[error("{0}")]
    Message(String),
}

impl OrganizationCliError {
    fn from_app_service(err: &AppServiceError) -> Self {
        let projection = map_organization_error(err);
        Self::Taxonomy {
            code: projection.body.code.code().to_owned(),
            summary: projection.body.summary,
        }
    }
}

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
        /// Page size for organization listing.
        #[arg(long, default_value_t = LIST_ORGANIZATIONS_DEFAULT_LIMIT)]
        limit: u64,
        /// Opaque pagination cursor from a prior list call.
        #[arg(long)]
        cursor: Option<String>,
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
) -> OrganizationResult<()> {
    match action {
        OrganizationAction::Create {
            database_url,
            account_id,
            name,
        } => create_organization(handlers, &database_url, &account_id, &name).await?,
        OrganizationAction::List {
            database_url,
            account_id,
            limit,
            cursor,
        } => {
            list_organizations(
                handlers,
                &database_url,
                &account_id,
                limit,
                cursor.as_deref(),
            )
            .await?;
        }
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
) -> OrganizationResult<()> {
    let store = Store::connect(database_url)
        .await
        .map_err(|e| OrganizationCliError::Message(format!("connect to store: {e}")))?;
    let account_id = parse_account_id(account_id_raw)?;
    let session_token = read_session_token()?;
    let name = OrganizationName::parse(name_raw).map_err(|e| {
        OrganizationCliError::Message(format!("parse --name as organization name: {e}"))
    })?;
    let response = handlers
        .create_organization(
            &store,
            CreateOrganizationRequest::from_api(
                session_token,
                account_id,
                tanren_contract::CreateOrganizationApiRequest::new(name, None),
            ),
        )
        .await
        .map_err(|err| OrganizationCliError::from_app_service(&err))?;
    let granted = response
        .granted_permissions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let capabilities = response
        .capabilities
        .iter()
        .map(|capability| {
            format!(
                "{}:{}:{}",
                capability.permission, capability.key, capability.allowed
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let source_event = format!(
        "{}.{}",
        response.source_link.event_family, response.source_link.event_kind
    );
    let source_event_id = response
        .source_event
        .as_ref()
        .map_or_else(|| "<none>".to_owned(), |event| event.event_id.clone());
    let source_event_cursor = response
        .source_event
        .as_ref()
        .map_or_else(|| "<none>".to_owned(), |event| event.cursor.clone());
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "organization_id={id} name={name} granted_permissions={granted} initial_project_count={initial_project_count} proof_behavior_id={proof_behavior_id} source_event={source_event} source_event_id={source_event_id} source_event_cursor={source_event_cursor} capabilities={capabilities}",
        id = response.organization.id,
        name = response.organization.name,
        granted = granted,
        initial_project_count = response.initial_project_count,
        proof_behavior_id = response.proof_link.behavior_id,
        source_event = source_event,
        source_event_id = source_event_id,
        source_event_cursor = source_event_cursor,
        capabilities = capabilities,
    )
    .map_err(|e| OrganizationCliError::Message(format!("write create-organization result: {e}")))?;
    Ok(())
}

async fn list_organizations(
    handlers: &Handlers,
    database_url: &str,
    account_id_raw: &str,
    limit: u64,
    cursor_raw: Option<&str>,
) -> OrganizationResult<()> {
    let store = Store::connect(database_url)
        .await
        .map_err(|e| OrganizationCliError::Message(format!("connect to store: {e}")))?;
    let account_id = parse_account_id(account_id_raw)?;
    let session_token = read_session_token()?;
    let cursor = parse_cursor(cursor_raw)?;
    let response = handlers
        .list_organizations(
            &store,
            ListOrganizationsRequest::from_api_query(
                session_token,
                account_id,
                &tanren_contract::ListOrganizationsApiQuery {
                    limit: Some(limit),
                    cursor,
                },
            ),
        )
        .await
        .map_err(|err| OrganizationCliError::from_app_service(&err))?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let next_cursor = response
        .next_cursor
        .map_or_else(|| "<none>".to_owned(), |cursor| cursor.to_string());
    let freshness_cursor = response
        .freshness
        .cursor
        .clone()
        .unwrap_or_else(|| "<none>".to_owned());
    let freshness_checkpoint = response
        .freshness
        .checkpoint
        .clone()
        .unwrap_or_else(|| "<none>".to_owned());
    if response.organizations.is_empty() {
        writeln!(
            handle,
            "organizations=0 next_cursor={next_cursor} freshness_projection={} freshness_generated_at={} freshness_cursor={freshness_cursor} freshness_checkpoint={freshness_checkpoint}",
            response.freshness.projection,
            response.freshness.generated_at.to_rfc3339(),
        )
        .map_err(|e| {
            OrganizationCliError::Message(format!("write organization-list result: {e}"))
        })?;
    } else {
        writeln!(
            handle,
            "organizations={} next_cursor={next_cursor} freshness_projection={} freshness_generated_at={} freshness_cursor={freshness_cursor} freshness_checkpoint={freshness_checkpoint}",
            response.organizations.len(),
            response.freshness.projection,
            response.freshness.generated_at.to_rfc3339(),
        )
        .map_err(|e| {
            OrganizationCliError::Message(format!("write organization-list count: {e}"))
        })?;
        for org in response.organizations {
            let capabilities = org
                .capabilities
                .iter()
                .map(|capability| {
                    format!(
                        "{}:{}:{}",
                        capability.permission, capability.key, capability.allowed
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            writeln!(
                handle,
                "organization_id={} name={} capabilities={}",
                org.id, org.name, capabilities
            )
            .map_err(|e| {
                OrganizationCliError::Message(format!("write organization-list row: {e}"))
            })?;
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
) -> OrganizationResult<()> {
    let store = Store::connect(database_url)
        .await
        .map_err(|e| OrganizationCliError::Message(format!("connect to store: {e}")))?;
    let account_id = parse_account_id(account_id_raw)?;
    let org_id = parse_org_id(org_id_raw)?;
    let permission = parse_permission(permission_raw)?;
    let session_token = read_session_token()?;
    let response = handlers
        .check_organization_permission(
            &store,
            CheckOrganizationPermissionRequest::from_api(
                session_token,
                account_id,
                &tanren_contract::CheckOrganizationPermissionApiRequest::new(org_id, permission),
            ),
        )
        .await
        .map_err(|err| OrganizationCliError::from_app_service(&err))?;
    if !response.allowed {
        return Err(OrganizationCliError::from_app_service(
            &AppServiceError::Account(AccountFailureReason::PermissionDenied),
        ));
    }
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "account_id={account} org_id={org} permission={permission} allowed=true",
        account = response.account_id,
        org = response.org_id,
        permission = response.permission,
    )
    .map_err(|e| OrganizationCliError::Message(format!("write permission-check result: {e}")))?;
    Ok(())
}

fn read_session_token() -> OrganizationResult<SessionToken> {
    let path = session_path();
    let token = match fs::read_to_string(&path) {
        Ok(token) => token,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(OrganizationCliError::from_app_service(
                &AppServiceError::Account(AccountFailureReason::AuthRequired),
            ));
        }
        Err(err) => {
            return Err(OrganizationCliError::Message(format!(
                "read session from {}: {err}",
                path.display()
            )));
        }
    };
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(OrganizationCliError::from_app_service(
            &AppServiceError::Account(AccountFailureReason::AuthRequired),
        ));
    }
    Ok(SessionToken::from_secret(SecretString::from(
        trimmed.to_owned(),
    )))
}

fn parse_account_id(raw: &str) -> OrganizationResult<AccountId> {
    let uuid = Uuid::parse_str(raw)
        .map_err(|e| OrganizationCliError::Message(format!("parse --account-id as uuid: {e}")))?;
    Ok(AccountId::from(uuid))
}

fn parse_org_id(raw: &str) -> OrganizationResult<OrgId> {
    let uuid = Uuid::parse_str(raw)
        .map_err(|e| OrganizationCliError::Message(format!("parse --org-id as uuid: {e}")))?;
    Ok(OrgId::from(uuid))
}

fn parse_permission(raw: &str) -> OrganizationResult<OrganizationPermission> {
    OrganizationPermission::from_str(raw.trim()).map_err(|_| {
        let expected = OrganizationPermission::ALL
            .into_iter()
            .map(OrganizationPermission::as_str)
            .collect::<Vec<_>>()
            .join("|");
        OrganizationCliError::Message(format!("parse --permission: expected {expected}"))
    })
}

fn parse_cursor(raw: Option<&str>) -> OrganizationResult<Option<MembershipId>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let uuid = Uuid::parse_str(trimmed)
        .map_err(|e| OrganizationCliError::Message(format!("parse --cursor as uuid: {e}")))?;
    Ok(Some(MembershipId::from(uuid)))
}
