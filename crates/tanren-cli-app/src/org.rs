//! Organization CLI subcommands: `org create` and `org list`.

use std::fs;
use std::io::Write;

use anyhow::{Context, Result};
use secrecy::SecretString;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::CreateOrganizationRequest;
use tanren_identity_policy::{OrgName, SessionToken};

pub(super) async fn run_create(handlers: &Handlers, store: &Store, name: &str) -> Result<()> {
    let session = load_session()?;
    let org_name = OrgName::parse(name).context("parse --name as organization name")?;
    let request = CreateOrganizationRequest { name: org_name };
    let response = handlers
        .create_organization(store, &session, request)
        .await
        .map_err(org_error)?;
    let json = serde_json::to_string(&response).context("serialize CreateOrganizationResponse")?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{json}").context("write CreateOrganizationResponse")?;
    Ok(())
}

pub(super) async fn run_list(handlers: &Handlers, store: &Store) -> Result<()> {
    let session = load_session()?;
    let response = handlers
        .list_organizations(store, &session)
        .await
        .map_err(org_error)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    for org_view in &response.organizations {
        let json = serde_json::to_string(org_view).context("serialize OrganizationView")?;
        writeln!(handle, "{json}").context("write OrganizationView")?;
    }
    Ok(())
}

fn load_session() -> Result<SessionToken> {
    let path = crate::session_path();
    let Ok(token) = fs::read_to_string(&path) else {
        return Err(anyhow::anyhow!("code: unauthenticated"));
    };
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("code: unauthenticated"));
    }
    Ok(SessionToken::from_secret(SecretString::from(
        trimmed.to_owned(),
    )))
}

fn org_error(err: AppServiceError) -> anyhow::Error {
    match err {
        AppServiceError::Organization(reason) => {
            anyhow::anyhow!("code: {}", reason.code())
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
