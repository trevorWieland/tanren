use tanren_app_services::{Handlers, Store};
use tanren_identity_policy::AccountId;
use tokio::runtime::Runtime;

use crate::app::{OutcomeView, Screen};
use crate::ui::{
    create_organization_outcome, operation_label, parse_create_organization, parse_org_admin_probe,
    render_error,
};

pub(crate) struct Ctx<'a> {
    pub(crate) runtime: &'a Runtime,
    pub(crate) handlers: &'a Handlers,
    pub(crate) screen: &'a mut Screen,
    pub(crate) session_account_id: Option<AccountId>,
}

impl Ctx<'_> {
    fn require_session(&self) -> Result<AccountId, String> {
        self.session_account_id
            .ok_or_else(|| "auth_required: sign in first".to_owned())
    }
}

pub(crate) fn create_organization(ctx: &mut Ctx<'_>, store: &Store) {
    let parsed = {
        let Screen::CreateOrganization(state) = &*ctx.screen else {
            return;
        };
        parse_create_organization(state)
    };
    let request = match parsed {
        Ok(req) => req,
        Err(msg) => {
            if let Screen::CreateOrganization(s) = &mut *ctx.screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    let account_id = match ctx.require_session() {
        Ok(id) => id,
        Err(msg) => {
            if let Screen::CreateOrganization(s) = &mut *ctx.screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    match ctx.runtime.block_on(
        ctx.handlers
            .create_organization_for_account(store, account_id, request),
    ) {
        Ok(resp) => {
            *ctx.screen = Screen::Outcome(create_organization_outcome(&resp));
        }
        Err(reason) => {
            if let Screen::CreateOrganization(s) = &mut *ctx.screen {
                s.error = Some(render_error(reason));
            }
        }
    }
}

pub(crate) fn list_organizations(ctx: &mut Ctx<'_>, store: &Store) {
    let account_id = match ctx.require_session() {
        Ok(id) => id,
        Err(msg) => {
            if let Screen::ListOrganizations(s) = &mut *ctx.screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    match ctx
        .runtime
        .block_on(ctx.handlers.list_account_organizations(store, account_id))
    {
        Ok(orgs) => {
            let mut lines = vec!["Available organizations:".to_owned()];
            if orgs.is_empty() {
                lines.push("(none)".to_owned());
            } else {
                for org in &orgs {
                    lines.push(format!(
                        "org_id={} name={} project_count={}",
                        org.id, org.display_name, org.project_count
                    ));
                }
            }
            *ctx.screen = Screen::Outcome(OutcomeView {
                title: "Organizations",
                lines,
            });
        }
        Err(reason) => {
            if let Screen::ListOrganizations(s) = &mut *ctx.screen {
                s.error = Some(render_error(reason));
            }
        }
    }
}

pub(crate) fn admin_probe(ctx: &mut Ctx<'_>, store: &Store) {
    let parsed = {
        let Screen::OrgAdminProbe(state) = &*ctx.screen else {
            return;
        };
        parse_org_admin_probe(state)
    };
    let (org_id, operation) = match parsed {
        Ok(vals) => vals,
        Err(msg) => {
            if let Screen::OrgAdminProbe(s) = &mut *ctx.screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    let account_id = match ctx.require_session() {
        Ok(id) => id,
        Err(msg) => {
            if let Screen::OrgAdminProbe(s) = &mut *ctx.screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    match ctx.runtime.block_on(
        ctx.handlers
            .authorize_org_admin_operation(store, account_id, org_id, operation),
    ) {
        Ok(()) => {
            *ctx.screen = Screen::Outcome(OutcomeView {
                title: "Authorized",
                lines: vec![
                    format!("org_id: {org_id}"),
                    format!("operation: {}", operation_label(operation)),
                    "authorized: true".to_owned(),
                ],
            });
        }
        Err(reason) => {
            if let Screen::OrgAdminProbe(s) = &mut *ctx.screen {
                s.error = Some(render_error(reason));
            }
        }
    }
}
