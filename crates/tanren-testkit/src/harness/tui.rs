//! `@tui` harness — drives the `tanren-tui` binary inside a
//! pseudo-terminal via `expectrl`.
//!
//! Account-flow actions and fixture seeding route through [`Handlers`] /
//! [`Store`] as setup plumbing. Project-flow *actions* (list, switch,
//! attention drill-down, scoped views) drive the real `tanren-tui`
//! binary: each action spawns the binary in a fresh PTY, signs in,
//! navigates via keyboard, scrapes the rendered screen, and parses
//! the result back into contract types.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use tanren_app_services::{AppServiceError, Clock, Handlers, Store};
use tanren_contract::{
    AcceptInvitationRequest, AttentionSpecView, ProjectScopedViews, ProjectView, SignInRequest,
    SignUpRequest, SwitchProjectResponse,
};
use tanren_identity_policy::{AccountId, Argon2idVerifier, ProjectId, SpecId};
use tanren_store::{AccountStore, EventEnvelope, NewInvitation, NewProject, NewSpec, ProjectStore};

use super::api_support::{locate_workspace_binary, scenario_db_path, sqlite_url};
use super::project::{HarnessProjectFixture, HarnessSpecFixture, ProjectHarness};
use super::tui_support::{
    build_switch_response, drive_attention_spec, drive_project_list, drive_scoped_views,
    drive_switch_project, parse_attention_spec, parse_project_list, read_active_project_scoped,
};
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind, HarnessResult,
    HarnessSession,
};

#[derive(Clone)]
struct TuiAccount {
    account_id: AccountId,
    email: String,
    password: String,
}

pub struct TuiHarness {
    store: Arc<Store>,
    handlers: Handlers,
    db_path: PathBuf,
    db_url: String,
    binary: PathBuf,
    account_map: HashMap<AccountId, TuiAccount>,
    project_names: HashMap<ProjectId, String>,
    spec_data: HashMap<SpecId, (String, Option<String>)>,
}

impl std::fmt::Debug for TuiHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TuiHarness")
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl TuiHarness {
    pub async fn spawn() -> HarnessResult<Self> {
        let db_path = scenario_db_path("tui");
        let db_url = sqlite_url(&db_path);
        let store = Store::connect(&db_url)
            .await
            .map_err(|e| HarnessError::Transport(format!("connect: {e}")))?;
        store
            .migrate()
            .await
            .map_err(|e| HarnessError::Transport(format!("migrate: {e}")))?;
        let clock = Clock::from_fn(Utc::now);
        let handlers = Handlers::with_verifier(clock, Arc::new(Argon2idVerifier::fast_for_tests()));
        let binary = locate_workspace_binary("tanren-tui")?;
        Ok(Self {
            store: Arc::new(store),
            handlers,
            db_path,
            db_url,
            binary,
            account_map: HashMap::new(),
            project_names: HashMap::new(),
            spec_data: HashMap::new(),
        })
    }

    async fn ensure_tui_account(&mut self, step_aid: AccountId) -> HarnessResult<()> {
        if self.account_map.contains_key(&step_aid) {
            return Ok(());
        }
        let idx = self.account_map.len();
        let email_str = format!("tui-{idx}@tanren.test");
        let pw_str = format!("tui-pw-{idx}");
        let email = tanren_identity_policy::Email::parse(&email_str)
            .map_err(|e| HarnessError::Transport(format!("email: {e}")))?;
        let resp = self
            .handlers
            .sign_up(
                self.store.as_ref(),
                SignUpRequest {
                    email,
                    password: secrecy::SecretString::from(pw_str.clone()),
                    display_name: format!("TUI Test {idx}"),
                },
            )
            .await
            .map_err(translate_app_error)?;
        self.account_map.insert(
            step_aid,
            TuiAccount {
                account_id: resp.account.id,
                email: email_str,
                password: pw_str,
            },
        );
        Ok(())
    }

    fn require_account(&self, step_aid: AccountId) -> HarnessResult<&TuiAccount> {
        self.account_map
            .get(&step_aid)
            .ok_or_else(|| HarnessError::Transport("no TUI account for step id".to_owned()))
    }
}

impl Drop for TuiHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.db_path);
    }
}

#[async_trait]
impl AccountHarness for TuiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Tui
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        match self.handlers.sign_up(self.store.as_ref(), req).await {
            Ok(r) => Ok(HarnessSession {
                account: r.account.clone(),
                account_id: r.account.id,
                expires_at: r.session.expires_at,
                has_token: !r.session.token.expose_secret().is_empty(),
            }),
            Err(e) => Err(translate_app_error(e)),
        }
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        match self.handlers.sign_in(self.store.as_ref(), req).await {
            Ok(r) => Ok(HarnessSession {
                account: r.account.clone(),
                account_id: r.account.id,
                expires_at: r.session.expires_at,
                has_token: !r.session.token.expose_secret().is_empty(),
            }),
            Err(e) => Err(translate_app_error(e)),
        }
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        match self
            .handlers
            .accept_invitation(self.store.as_ref(), req)
            .await
        {
            Ok(r) => Ok(HarnessAcceptance {
                session: HarnessSession {
                    account: r.account.clone(),
                    account_id: r.account.id,
                    expires_at: r.session.expires_at,
                    has_token: !r.session.token.expose_secret().is_empty(),
                },
                joined_org: r.joined_org,
            }),
            Err(e) => Err(translate_app_error(e)),
        }
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.store
            .seed_invitation(NewInvitation {
                token: fixture.token,
                inviting_org_id: fixture.inviting_org,
                expires_at: fixture.expires_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed: {e}")))?;
        Ok(())
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        AccountStore::recent_events(self.store.as_ref(), limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("events: {e}")))
    }
}

#[async_trait]
impl ProjectHarness for TuiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Tui
    }

    async fn seed_project(&mut self, f: HarnessProjectFixture) -> HarnessResult<ProjectId> {
        let id = f.id;
        self.project_names.insert(id, f.name.clone());
        self.ensure_tui_account(f.account_id).await?;
        let tui_aid = self.require_account(f.account_id)?.account_id;
        self.store
            .seed_project(NewProject {
                id,
                account_id: tui_aid,
                name: f.name,
                state: f.state,
                created_at: f.created_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_project: {e}")))?;
        Ok(id)
    }

    async fn seed_spec(&mut self, f: HarnessSpecFixture) -> HarnessResult<SpecId> {
        let id = f.id;
        self.spec_data
            .insert(id, (f.name.clone(), f.attention_reason.clone()));
        self.store
            .seed_spec(NewSpec {
                id,
                project_id: f.project_id,
                name: f.name,
                needs_attention: f.needs_attention,
                attention_reason: f.attention_reason,
                created_at: f.created_at,
            })
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_spec: {e}")))?;
        Ok(id)
    }

    async fn seed_view_state(
        &mut self,
        aid: AccountId,
        pid: ProjectId,
        state: Value,
    ) -> HarnessResult<()> {
        self.ensure_tui_account(aid).await?;
        let tui_aid = self.require_account(aid)?.account_id;
        self.store
            .write_view_state(tui_aid, pid, state, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("view_state: {e}")))?;
        Ok(())
    }

    async fn list_projects(&mut self, step_aid: AccountId) -> HarnessResult<Vec<ProjectView>> {
        let acct = self.require_account(step_aid)?.clone();
        let binary = self.binary.clone();
        let db_url = self.db_url.clone();
        let pnames = self.project_names.clone();
        let sdata = self.spec_data.clone();

        tokio::task::spawn_blocking(move || {
            let screen = drive_project_list(&binary, &db_url, &acct.email, &acct.password)?;
            Ok(parse_project_list(&screen, &pnames, &sdata))
        })
        .await
        .map_err(|e| HarnessError::Transport(format!("join: {e}")))?
    }

    async fn switch_active_project(
        &mut self,
        step_aid: AccountId,
        pid: ProjectId,
    ) -> HarnessResult<SwitchProjectResponse> {
        let acct = self.require_account(step_aid)?.clone();
        let pname = self
            .project_names
            .get(&pid)
            .cloned()
            .ok_or_else(|| HarnessError::Transport("unknown project id".to_owned()))?;
        let binary = self.binary.clone();
        let db_url = self.db_url.clone();
        let pnames = self.project_names.clone();
        let sdata = self.spec_data.clone();
        let store = self.store.clone();

        tokio::task::spawn_blocking(move || {
            let screen =
                drive_switch_project(&binary, &db_url, &acct.email, &acct.password, &pname)?;
            build_switch_response(&screen, pid, &pnames, &sdata, store.as_ref())
        })
        .await
        .map_err(|e| HarnessError::Transport(format!("join: {e}")))?
    }

    async fn attention_spec(
        &mut self,
        step_aid: AccountId,
        _pid: ProjectId,
        sid: SpecId,
    ) -> HarnessResult<AttentionSpecView> {
        let acct = self.require_account(step_aid)?.clone();
        let sname = self
            .spec_data
            .get(&sid)
            .map(|(n, _)| n.clone())
            .ok_or_else(|| HarnessError::Transport("unknown spec id".to_owned()))?;
        let binary = self.binary.clone();
        let db_url = self.db_url.clone();

        tokio::task::spawn_blocking(move || {
            let screen =
                drive_attention_spec(&binary, &db_url, &acct.email, &acct.password, &sname)?;
            Ok(parse_attention_spec(&screen, sid, &sname))
        })
        .await
        .map_err(|e| HarnessError::Transport(format!("join: {e}")))?
    }

    async fn project_scoped_views(
        &mut self,
        step_aid: AccountId,
    ) -> HarnessResult<ProjectScopedViews> {
        let acct = self.require_account(step_aid)?.clone();
        let store = self.store.clone();
        let binary = self.binary.clone();
        let db_url = self.db_url.clone();

        tokio::task::spawn_blocking(move || {
            let _screen = drive_scoped_views(&binary, &db_url, &acct.email, &acct.password)?;
            read_active_project_scoped(store.as_ref(), acct.account_id)
        })
        .await
        .map_err(|e| HarnessError::Transport(format!("join: {e}")))?
    }
}

fn translate_app_error(err: AppServiceError) -> HarnessError {
    match err {
        AppServiceError::Account(r, ..) => HarnessError::Account(r, r.code().to_owned()),
        AppServiceError::Project(r, ..) => HarnessError::Project(r, r.code().to_owned()),
        AppServiceError::InvalidInput(m) => HarnessError::Transport(format!("input: {m}")),
        AppServiceError::Store(e) => HarnessError::Transport(format!("store: {e}")),
        _ => HarnessError::Transport("unknown".to_owned()),
    }
}
