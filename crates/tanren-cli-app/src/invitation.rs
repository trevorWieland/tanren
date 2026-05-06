use std::fs;
use std::io::Write;

use anyhow::{Context, Result, bail};
use chrono::{Duration, Utc};
use clap::Subcommand;
use secrecy::SecretString;
use tanren_app_services::{AccountStore, Handlers, Store};
use tanren_contract::{CreateOrgInvitationRequest, InvitationStatus, RevokeOrgInvitationRequest};
use tanren_identity_policy::{
    AccountId, Identifier, InvitationToken, OrgId, OrganizationPermission, SessionToken,
};
use uuid::Uuid;

#[derive(Debug, Subcommand)]
pub(crate) enum InvitationAction {
    /// Send an organization invitation.
    Send {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        org_id: String,
        #[arg(long)]
        recipient_identifier: String,
        #[arg(long, num_args(1..))]
        permissions: Vec<String>,
        #[arg(long, default_value_t = 30)]
        expires_in_days: i64,
        #[arg(long)]
        session: Option<String>,
    },
    /// List invitations by org or recipient identifier.
    List {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        org_id: Option<String>,
        #[arg(long)]
        recipient_identifier: Option<String>,
        #[arg(long)]
        session: Option<String>,
    },
    /// Revoke a pending invitation.
    Revoke {
        #[arg(long, env = "DATABASE_URL")]
        database_url: String,
        #[arg(long)]
        org_id: String,
        #[arg(long)]
        token: String,
        #[arg(long)]
        session: Option<String>,
    },
}

pub(crate) fn dispatch_invitation(action: InvitationAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    runtime.block_on(run_invitation(action))
}

async fn run_invitation(action: InvitationAction) -> Result<()> {
    let handlers = Handlers::new();
    match action {
        a @ InvitationAction::Send { .. } => run_invitation_send(&handlers, a).await,
        a @ InvitationAction::List { .. } => run_invitation_list(&handlers, a).await,
        a @ InvitationAction::Revoke { .. } => run_invitation_revoke(&handlers, a).await,
    }
}

async fn run_invitation_send(handlers: &Handlers, action: InvitationAction) -> Result<()> {
    let InvitationAction::Send {
        database_url,
        org_id,
        recipient_identifier,
        permissions,
        expires_in_days,
        session,
    } = action
    else {
        bail!("internal error: wrong action variant");
    };
    let store = Store::connect(&database_url)
        .await
        .context("connect to store")?;
    let account_id = resolve_account_id(&store, session.as_deref()).await?;
    let org_id = OrgId::new(Uuid::parse_str(&org_id).context("parse --org-id as UUID")?);
    let recipient =
        Identifier::parse(&recipient_identifier).context("parse --recipient-identifier")?;
    let perms: Vec<OrganizationPermission> = permissions
        .iter()
        .map(|p| OrganizationPermission::parse(p))
        .collect::<Result<Vec<_>, _>>()
        .context("parse --permissions")?;
    let request = CreateOrgInvitationRequest {
        org_id,
        recipient_identifier: recipient,
        permissions: perms,
        expires_at: Utc::now() + Duration::days(expires_in_days),
    };
    let inv = &handlers
        .create_invitation(&store, account_id, Some(org_id), request)
        .await
        .map_err(super::account_error)?
        .invitation;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "invitation_token={} org_id={} recipient_identifier={} permissions={} status={} expires_at={}",
        inv.token,
        inv.org_id,
        inv.recipient_identifier,
        inv.permissions.iter().map(OrganizationPermission::as_str).collect::<Vec<_>>().join(","),
        format_invitation_status(inv.status),
        inv.expires_at.to_rfc3339(),
    )
    .context("write invitation send result")
}

async fn run_invitation_list(handlers: &Handlers, action: InvitationAction) -> Result<()> {
    let InvitationAction::List {
        database_url,
        org_id,
        recipient_identifier,
        session,
    } = action
    else {
        bail!("internal error: wrong action variant");
    };
    let store = Store::connect(&database_url)
        .await
        .context("connect to store")?;
    let response = match (org_id, recipient_identifier) {
        (Some(org_id_str), None) => {
            let account_id = resolve_account_id(&store, session.as_deref()).await?;
            let org_id =
                OrgId::new(Uuid::parse_str(&org_id_str).context("parse --org-id as UUID")?);
            handlers
                .list_org_invitations(&store, account_id, org_id)
                .await
                .map_err(super::account_error)?
        }
        (None, Some(recipient)) => {
            let identifier =
                Identifier::parse(&recipient).context("parse --recipient-identifier")?;
            handlers
                .list_recipient_invitations(&store, &identifier)
                .await
                .map_err(super::account_error)?
        }
        (Some(_), Some(_)) => {
            bail!(
                "error: validation_failed — specify --org-id or --recipient-identifier, not both"
            );
        }
        (None, None) => {
            bail!("error: validation_failed — specify --org-id or --recipient-identifier");
        }
    };
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "invitations={}", response.invitations.len())
        .context("write invitation list header")?;
    for inv in &response.invitations {
        writeln!(
            handle,
            "token={} org_id={} recipient_identifier={} permissions={} status={}",
            inv.token,
            inv.org_id,
            inv.recipient_identifier,
            inv.permissions
                .iter()
                .map(OrganizationPermission::as_str)
                .collect::<Vec<_>>()
                .join(","),
            format_invitation_status(inv.status),
        )
        .context("write invitation list entry")?;
    }
    Ok(())
}

async fn run_invitation_revoke(handlers: &Handlers, action: InvitationAction) -> Result<()> {
    let InvitationAction::Revoke {
        database_url,
        org_id,
        token,
        session,
    } = action
    else {
        bail!("internal error: wrong action variant");
    };
    let store = Store::connect(&database_url)
        .await
        .context("connect to store")?;
    let account_id = resolve_account_id(&store, session.as_deref()).await?;
    let org_id = OrgId::new(Uuid::parse_str(&org_id).context("parse --org-id as UUID")?);
    let token = InvitationToken::parse(&token).context("parse --token")?;
    let request = RevokeOrgInvitationRequest { org_id, token };
    let inv = &handlers
        .revoke_invitation(&store, account_id, Some(org_id), request)
        .await
        .map_err(super::account_error)?
        .invitation;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(
        handle,
        "invitation_token={} org_id={} status={}",
        inv.token,
        inv.org_id,
        format_invitation_status(inv.status),
    )
    .context("write invitation revoke result")
}

fn format_invitation_status(status: InvitationStatus) -> &'static str {
    match status {
        InvitationStatus::Pending => "pending",
        InvitationStatus::Accepted => "accepted",
        InvitationStatus::Revoked => "revoked",
        _ => "unknown",
    }
}

async fn resolve_account_id<S>(store: &S, explicit_session: Option<&str>) -> Result<AccountId>
where
    S: AccountStore + ?Sized,
{
    let token_str = match explicit_session {
        Some(s) if !s.is_empty() => s.to_owned(),
        _ => {
            let path = super::session_path();
            fs::read_to_string(&path)
                .with_context(|| format!("read session from {}", path.display()))?
        }
    };
    let session_token = SessionToken::from_secret(SecretString::from(token_str));
    let record = store
        .find_session_by_token(&session_token, Utc::now())
        .await
        .context("lookup session")?
        .ok_or_else(|| anyhow::anyhow!("error: unauthenticated — session not found or expired"))?;
    Ok(record.account_id)
}
