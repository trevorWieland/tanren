//! `@tui` harness — drives the real `tanren-tui` binary over a pty.
use super::cli_support::locate_workspace_binary;
use super::common::{scenario_db_path, spawn_api_server, sqlite_url};
use super::tui_support::{
    AccountCredentials, RE_ACCEPT_SUCCESS, RE_CHECK_SUCCESS, RE_CREATE_SUCCESS, RE_ERROR_ACCEPT,
    RE_ERROR_CHECK, RE_ERROR_CREATE, RE_ERROR_LIST, RE_ERROR_SIGN_IN, RE_ERROR_SIGN_UP,
    RE_LIST_ROW, RE_LIST_SUCCESS, RE_SIGN_IN_SUCCESS, RE_SIGN_UP_SUCCESS, READY_MARKER,
    TUI_EXPECT_TIMEOUT, build_create_organization_response, build_list_organization_view,
    build_list_organizations_response, close_session, expect_five_regex_captures, expect_literal,
    expect_regex_capture, expect_three_regex_captures, expect_two_regex_captures, open_form,
    parse_account_id, parse_count, parse_initial_project_count, parse_org_id, parse_permissions,
    parse_reason_code, parse_source_event, send, sign_in_in_session,
};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};
use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use secrecy::ExposeSecret;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use std::sync::Arc;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountView, CheckOrganizationPermissionResponse,
    CreateOrganizationResponse, ListOrganizationsResponse, OrganizationBehaviorId, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::{
    AccountId, Identifier, OrgId, OrganizationName, OrganizationPermission,
};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation};
use tokio::task::JoinHandle;
/// `@tui` wire harness.
pub struct TuiHarness {
    store: Arc<Store>,
    db_path: PathBuf,
    api_base_url: String,
    server: Option<JoinHandle<()>>,
    binary: PathBuf,
    credentials_by_account: HashMap<AccountId, AccountCredentials>,
}
impl std::fmt::Debug for TuiHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TuiHarness")
            .field("db_path", &self.db_path)
            .field("binary", &self.binary)
            .finish_non_exhaustive()
    }
}
impl TuiHarness {
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = scenario_db_path("tui");
        let db_url = sqlite_url(&db_path);
        let store = Store::connect(&db_url)
            .await
            .map_err(|e| HarnessError::Transport(format!("connect store: {e}")))?;
        store
            .migrate()
            .await
            .map_err(|e| HarnessError::Transport(format!("migrate store: {e}")))?;
        let store = Arc::new(store);
        let (api_base_url, server) = spawn_api_server(store.clone(), &db_url).await?;
        let binary = locate_workspace_binary("tanren-tui")?;
        Ok(Self {
            store,
            db_path,
            api_base_url,
            server: Some(server),
            binary,
            credentials_by_account: HashMap::new(),
        })
    }
    fn spawn_session(&self) -> HarnessResult<expectrl::Session> {
        let _pty_system = portable_pty::native_pty_system();
        let mut command = Command::new(&self.binary);
        command.env("TANREN_API_BASE_URL", &self.api_base_url);
        command.env("RUST_LOG", "info");
        let mut session = expectrl::Session::spawn(command)
            .map_err(|e| HarnessError::Transport(format!("spawn tui: {e}")))?;
        session.set_expect_timeout(Some(TUI_EXPECT_TIMEOUT));
        expect_literal(&mut session, READY_MARKER, "startup readiness marker")?;
        Ok(session)
    }
}
impl Drop for TuiHarness {
    fn drop(&mut self) {
        if let Some(handle) = self.server.take() {
            handle.abort();
        }
        let _ = std::fs::remove_file(&self.db_path);
    }
}
#[async_trait]
impl AccountHarness for TuiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Tui
    }
    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let mut session = self.spawn_session()?;
        open_form(&mut session, 0, "open sign-up form")?;
        send(&mut session, req.email.as_str(), "fill sign-up email")?;
        send(&mut session, "\t", "sign-up next field")?;
        send(
            &mut session,
            req.password.expose_secret(),
            "fill sign-up password",
        )?;
        send(&mut session, "\t", "sign-up next field")?;
        send(&mut session, &req.display_name, "fill sign-up display name")?;
        send(&mut session, "\r", "submit sign-up form")?;
        let result = match expect_two_regex_captures(
            &mut session,
            RE_SIGN_UP_SUCCESS,
            1,
            2,
            "sign-up success marker",
        ) {
            Ok((account_id_raw, token_present_raw)) => {
                let account_id = parse_account_id(&account_id_raw, "sign-up")?;
                let has_token = token_present_raw.trim() == "true";
                let identifier = Identifier::from_email(&req.email);
                let account = AccountView {
                    id: account_id,
                    identifier,
                    display_name: req.display_name.clone(),
                    org: None,
                };
                self.credentials_by_account.insert(
                    account_id,
                    AccountCredentials {
                        email: req.email.as_str().to_owned(),
                        password: req.password.clone(),
                    },
                );
                Ok(HarnessSession {
                    account_id,
                    account,
                    expires_at: Utc::now() + ChronoDuration::days(30),
                    has_token,
                })
            }
            Err(success_err) => {
                if let Ok(code) =
                    expect_regex_capture(&mut session, RE_ERROR_SIGN_UP, 1, "sign-up error marker")
                {
                    let reason = parse_reason_code(&code)?;
                    Err(HarnessError::Account(reason, reason.summary().to_owned()))
                } else {
                    Err(success_err)
                }
            }
        };
        let _ = close_session(&mut session);
        result
    }
    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let mut session = self.spawn_session()?;
        open_form(&mut session, 1, "open sign-in form")?;
        send(&mut session, req.email.as_str(), "fill sign-in email")?;
        send(&mut session, "\t", "sign-in next field")?;
        send(
            &mut session,
            req.password.expose_secret(),
            "fill sign-in password",
        )?;
        send(&mut session, "\r", "submit sign-in form")?;
        let result = match expect_two_regex_captures(
            &mut session,
            RE_SIGN_IN_SUCCESS,
            1,
            2,
            "sign-in success marker",
        ) {
            Ok((account_id_raw, token_present_raw)) => {
                let account_id = parse_account_id(&account_id_raw, "sign-in")?;
                let has_token = token_present_raw.trim() == "true";
                let identifier = Identifier::from_email(&req.email);
                let account = AccountView {
                    id: account_id,
                    identifier,
                    display_name: String::new(),
                    org: None,
                };
                self.credentials_by_account.insert(
                    account_id,
                    AccountCredentials {
                        email: req.email.as_str().to_owned(),
                        password: req.password.clone(),
                    },
                );
                Ok(HarnessSession {
                    account_id,
                    account,
                    expires_at: Utc::now() + ChronoDuration::days(30),
                    has_token,
                })
            }
            Err(success_err) => {
                if let Ok(code) =
                    expect_regex_capture(&mut session, RE_ERROR_SIGN_IN, 1, "sign-in error marker")
                {
                    let reason = parse_reason_code(&code)?;
                    Err(HarnessError::Account(reason, reason.summary().to_owned()))
                } else {
                    Err(success_err)
                }
            }
        };
        let _ = close_session(&mut session);
        result
    }
    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let mut session = self.spawn_session()?;
        open_form(&mut session, 2, "open accept-invitation form")?;
        send(
            &mut session,
            req.invitation_token.as_str(),
            "fill invitation token",
        )?;
        send(&mut session, "\t", "invitation next field")?;
        send(&mut session, req.email.as_str(), "fill invitation email")?;
        send(&mut session, "\t", "invitation next field")?;
        send(
            &mut session,
            req.password.expose_secret(),
            "fill invitation password",
        )?;
        send(&mut session, "\t", "invitation next field")?;
        send(
            &mut session,
            &req.display_name,
            "fill invitation display name",
        )?;
        send(&mut session, "\r", "submit invitation form")?;
        let result = match expect_three_regex_captures(
            &mut session,
            RE_ACCEPT_SUCCESS,
            1,
            2,
            3,
            "accept-invitation success marker",
        ) {
            Ok((account_id_raw, joined_org_raw, token_present_raw)) => {
                let account_id = parse_account_id(&account_id_raw, "accept-invitation")?;
                let joined_org = parse_org_id(&joined_org_raw, "accept-invitation")?;
                let has_token = token_present_raw.trim() == "true";
                let identifier = Identifier::from_email(&req.email);
                let account = AccountView {
                    id: account_id,
                    identifier,
                    display_name: req.display_name.clone(),
                    org: Some(joined_org),
                };
                self.credentials_by_account.insert(
                    account_id,
                    AccountCredentials {
                        email: req.email.as_str().to_owned(),
                        password: req.password.clone(),
                    },
                );
                Ok(HarnessAcceptance {
                    session: HarnessSession {
                        account_id,
                        account,
                        expires_at: Utc::now() + ChronoDuration::days(30),
                        has_token,
                    },
                    joined_org,
                })
            }
            Err(success_err) => {
                if let Ok(code) = expect_regex_capture(
                    &mut session,
                    RE_ERROR_ACCEPT,
                    1,
                    "accept-invitation error marker",
                ) {
                    let reason = parse_reason_code(&code)?;
                    Err(HarnessError::Account(reason, reason.summary().to_owned()))
                } else {
                    Err(success_err)
                }
            }
        };
        let _ = close_session(&mut session);
        result
    }
    async fn create_organization(
        &mut self,
        account_id: AccountId,
        name: OrganizationName,
    ) -> HarnessResult<CreateOrganizationResponse> {
        let mut session = self.spawn_session()?;
        if let Some(credentials) = self.credentials_by_account.get(&account_id) {
            sign_in_in_session(&mut session, credentials)?;
        }
        open_form(&mut session, 3, "open create-organization form")?;
        send(&mut session, name.as_str(), "fill organization name")?;
        send(&mut session, "\r", "submit create-organization form")?;
        let result = match expect_five_regex_captures(
            &mut session,
            RE_CREATE_SUCCESS,
            [1, 2, 3, 4, 5],
            "create-organization success marker",
        ) {
            Ok((
                org_id_raw,
                permissions_raw,
                project_count_raw,
                proof_behavior_id,
                source_event_raw,
            )) => {
                let org_id = parse_org_id(&org_id_raw, "create-organization")?;
                let granted_permissions = parse_permissions(&permissions_raw)?;
                let initial_project_count = parse_initial_project_count(&project_count_raw)?;
                let (event_family, event_kind) = parse_source_event(&source_event_raw)?;
                let proof_behavior_id = OrganizationBehaviorId::from_str(proof_behavior_id.trim())
                    .map_err(|err| {
                        HarnessError::Transport(format!("parse proof_behavior_id: {err}"))
                    })?;
                Ok(build_create_organization_response(
                    org_id,
                    name,
                    granted_permissions,
                    initial_project_count,
                    proof_behavior_id,
                    event_family,
                    event_kind,
                ))
            }
            Err(success_err) => {
                if let Ok(code) = expect_regex_capture(
                    &mut session,
                    RE_ERROR_CREATE,
                    1,
                    "create-organization error marker",
                ) {
                    let reason = parse_reason_code(&code)?;
                    Err(HarnessError::Account(reason, reason.summary().to_owned()))
                } else {
                    Err(success_err)
                }
            }
        };
        let _ = close_session(&mut session);
        result
    }
    async fn list_organizations(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ListOrganizationsResponse> {
        let mut session = self.spawn_session()?;
        if let Some(credentials) = self.credentials_by_account.get(&account_id) {
            sign_in_in_session(&mut session, credentials)?;
        }
        open_form(&mut session, 4, "open list-organizations form")?;
        send(&mut session, "\r", "submit list-organizations form")?;
        let result = match expect_regex_capture(
            &mut session,
            RE_LIST_SUCCESS,
            1,
            "list-organizations success marker",
        ) {
            Ok(count_raw) => {
                let count = parse_count(&count_raw)?;
                let mut organizations = Vec::with_capacity(count);
                for idx in 0..count {
                    let (id_raw, name_raw) = expect_two_regex_captures(
                        &mut session,
                        RE_LIST_ROW,
                        1,
                        2,
                        &format!("list-organizations row #{idx}"),
                    )?;
                    organizations.push(build_list_organization_view(&id_raw, &name_raw)?);
                }
                Ok(build_list_organizations_response(organizations))
            }
            Err(success_err) => {
                if let Ok(code) = expect_regex_capture(
                    &mut session,
                    RE_ERROR_LIST,
                    1,
                    "list-organizations error marker",
                ) {
                    let reason = parse_reason_code(&code)?;
                    Err(HarnessError::Account(reason, reason.summary().to_owned()))
                } else {
                    Err(success_err)
                }
            }
        };
        let _ = close_session(&mut session);
        result
    }
    async fn check_organization_admin_permission(
        &mut self,
        account_id: AccountId,
        org_id: OrgId,
        permission: OrganizationPermission,
    ) -> HarnessResult<CheckOrganizationPermissionResponse> {
        let mut session = self.spawn_session()?;
        if let Some(credentials) = self.credentials_by_account.get(&account_id) {
            sign_in_in_session(&mut session, credentials)?;
        }
        open_form(&mut session, 5, "open check-permission form")?;
        send(
            &mut session,
            &org_id.to_string(),
            "fill check-permission org id",
        )?;
        send(&mut session, "\t", "check-permission next field")?;
        let permission_key = permission.to_string();
        send(
            &mut session,
            &permission_key,
            "fill check-permission permission",
        )?;
        send(&mut session, "\r", "submit check-permission form")?;
        let result = match expect_three_regex_captures(
            &mut session,
            RE_CHECK_SUCCESS,
            1,
            2,
            3,
            "check-permission success marker",
        ) {
            Ok((account_id_raw, org_id_raw, permission_raw)) => {
                let parsed_account = parse_account_id(&account_id_raw, "check-permission")?;
                let parsed_org = parse_org_id(&org_id_raw, "check-permission")?;
                let parsed_permission = OrganizationPermission::from_str(permission_raw.trim())
                    .map_err(|_| {
                        HarnessError::Transport(format!(
                            "unknown permission in check-permission success marker: {permission_raw}"
                        ))
                    })?;
                Ok(CheckOrganizationPermissionResponse {
                    account_id: parsed_account,
                    org_id: parsed_org,
                    permission: parsed_permission,
                    allowed: true,
                })
            }
            Err(success_err) => {
                if let Ok(code) = expect_regex_capture(
                    &mut session,
                    RE_ERROR_CHECK,
                    1,
                    "check-permission error marker",
                ) {
                    let reason = parse_reason_code(&code)?;
                    Err(HarnessError::Account(reason, reason.summary().to_owned()))
                } else {
                    Err(success_err)
                }
            }
        };
        let _ = close_session(&mut session);
        result
    }
    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.store
            .seed_invitation(NewInvitation {
                token: fixture.token,
                inviting_org_id: fixture.inviting_org,
                expires_at: fixture.expires_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_invitation: {e}")))?;
        Ok(())
    }
    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }
}
