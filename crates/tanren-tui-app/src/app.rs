//! TUI screen state machine and submit dispatch.
//!
//! Split out of `lib.rs` so the tui-app crate stays under the workspace
//! 500-line line-budget. Keeps the `App` struct and the submit methods;
//! rendering lives in `draw.rs`, form factories + outcome adapters in
//! `ui.rs`, input-event dispatch in `dispatch.rs`.

use std::env;
use std::io::Stdout;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tanren_app_services::{Handlers, Store};
use tanren_identity_policy::AccountId;
use tokio::runtime::Runtime;

use crate::FormState;
use crate::dispatch::{
    Effect, FormAction, FormKind, form_effect, handle_menu_key, is_ctrl_c, is_press,
};
use crate::draw;
use crate::ui::{
    accept_invitation_outcome, create_organization_outcome, list_organizations_outcome,
    org_admin_probe_outcome, parse_accept_invitation, parse_create_organization,
    parse_org_admin_probe, parse_sign_in, parse_sign_up, render_error, sign_in_outcome,
    sign_up_outcome,
};

const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Debug)]
pub(crate) enum Screen {
    Menu { selected: usize },
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
    CreateOrganization(FormState),
    ListOrganizations(FormState),
    OrgAdminProbe(FormState),
    Outcome(OutcomeView),
}

#[derive(Debug)]
pub(crate) struct OutcomeView {
    pub(crate) title: &'static str,
    pub(crate) lines: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct App {
    runtime: Runtime,
    handlers: Handlers,
    store: Option<Arc<Store>>,
    store_error: Option<String>,
    session_account_id: Option<AccountId>,
    screen: Screen,
}

impl App {
    pub(crate) fn new() -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("build tokio runtime")?;
        let (store, store_error) = match env::var(DATABASE_URL_ENV) {
            Ok(url) if !url.is_empty() => match runtime.block_on(Store::connect(&url)) {
                Ok(store) => (Some(Arc::new(store)), None),
                Err(err) => (None, Some(format!("store unavailable: {err}"))),
            },
            _ => (
                None,
                Some(format!("{DATABASE_URL_ENV} is not set; submit will fail.")),
            ),
        };
        Ok(Self {
            runtime,
            handlers: Handlers::new(),
            store,
            store_error,
            session_account_id: None,
            screen: Screen::Menu { selected: 0 },
        })
    }

    pub(crate) fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        loop {
            terminal
                .draw(|frame| self.draw(frame))
                .context("render frame")?;
            if !event::poll(Duration::from_millis(200)).context("poll terminal events")? {
                continue;
            }
            let Event::Key(key) = event::read().context("read terminal event")? else {
                continue;
            };
            if !is_press(&key) {
                continue;
            }
            if self.handle_key(key) {
                return Ok(());
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if is_ctrl_c(&key) {
            return true;
        }
        let effect = match &mut self.screen {
            Screen::Menu { selected } => {
                let mut next: Option<Screen> = None;
                let exit = handle_menu_key(selected, key, &mut next);
                if exit {
                    Effect::Exit
                } else {
                    next.map_or(Effect::None, Effect::ReplaceScreen)
                }
            }
            Screen::Outcome(_) => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | 'Q')
                ) {
                    Effect::ReplaceScreen(Screen::Menu { selected: 0 })
                } else {
                    Effect::None
                }
            }
            Screen::SignUp(s) => form_effect(s, key, FormKind::SignUp),
            Screen::SignIn(s) => form_effect(s, key, FormKind::SignIn),
            Screen::AcceptInvitation(s) => form_effect(s, key, FormKind::AcceptInvitation),
            Screen::CreateOrganization(s) => form_effect(s, key, FormKind::CreateOrganization),
            Screen::ListOrganizations(s) => form_effect(s, key, FormKind::ListOrganizations),
            Screen::OrgAdminProbe(s) => form_effect(s, key, FormKind::OrgAdminProbe),
        };
        match effect {
            Effect::None => false,
            Effect::Exit => true,
            Effect::ReplaceScreen(s) => {
                self.screen = s;
                false
            }
            Effect::Form(action, kind) => {
                self.dispatch_form_action(action, kind);
                false
            }
        }
    }

    fn dispatch_form_action(&mut self, action: FormAction, kind: FormKind) {
        match action {
            FormAction::Cancel => {
                self.screen = Screen::Menu { selected: 0 };
            }
            FormAction::Submit => self.submit(kind),
        }
    }

    fn submit(&mut self, kind: FormKind) {
        let Some(store) = self.store.clone() else {
            let message = self
                .store_error
                .clone()
                .unwrap_or_else(|| "store unavailable".to_owned());
            if let Some(state) = self.active_form_mut() {
                state.error = Some(message);
            }
            return;
        };
        match kind {
            FormKind::SignUp => self.submit_sign_up(&store),
            FormKind::SignIn => self.submit_sign_in(&store),
            FormKind::AcceptInvitation => self.submit_accept_invitation(&store),
            FormKind::CreateOrganization => self.submit_create_organization(&store),
            FormKind::ListOrganizations => self.submit_list_organizations(&store),
            FormKind::OrgAdminProbe => self.submit_org_admin_probe(&store),
        }
    }

    fn submit_sign_up(&mut self, store: &Store) {
        let parsed = {
            let Screen::SignUp(state) = &self.screen else {
                return;
            };
            parse_sign_up(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(msg) => {
                if let Screen::SignUp(s) = &mut self.screen {
                    s.error = Some(msg);
                }
                return;
            }
        };
        match self.runtime.block_on(self.handlers.sign_up(store, request)) {
            Ok(resp) => {
                self.session_account_id = Some(resp.account.id);
                self.screen = Screen::Outcome(sign_up_outcome(&resp));
            }
            Err(reason) => {
                if let Screen::SignUp(s) = &mut self.screen {
                    s.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_sign_in(&mut self, store: &Store) {
        let parsed = {
            let Screen::SignIn(state) = &self.screen else {
                return;
            };
            parse_sign_in(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(msg) => {
                if let Screen::SignIn(s) = &mut self.screen {
                    s.error = Some(msg);
                }
                return;
            }
        };
        match self.runtime.block_on(self.handlers.sign_in(store, request)) {
            Ok(resp) => {
                self.session_account_id = Some(resp.account.id);
                self.screen = Screen::Outcome(sign_in_outcome(&resp));
            }
            Err(reason) => {
                if let Screen::SignIn(s) = &mut self.screen {
                    s.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_accept_invitation(&mut self, store: &Store) {
        let parsed = {
            let Screen::AcceptInvitation(state) = &self.screen else {
                return;
            };
            parse_accept_invitation(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(msg) => {
                if let Screen::AcceptInvitation(s) = &mut self.screen {
                    s.error = Some(msg);
                }
                return;
            }
        };
        match self
            .runtime
            .block_on(self.handlers.accept_invitation(store, request))
        {
            Ok(resp) => {
                self.session_account_id = Some(resp.account.id);
                self.screen = Screen::Outcome(accept_invitation_outcome(&resp));
            }
            Err(reason) => {
                if let Screen::AcceptInvitation(s) = &mut self.screen {
                    s.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_create_organization(&mut self, store: &Store) {
        let parsed = {
            let Screen::CreateOrganization(state) = &self.screen else {
                return;
            };
            parse_create_organization(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(msg) => {
                if let Screen::CreateOrganization(s) = &mut self.screen {
                    s.error = Some(msg);
                }
                return;
            }
        };
        let account_id = match self.require_session_account_id() {
            Ok(id) => id,
            Err(msg) => {
                if let Screen::CreateOrganization(s) = &mut self.screen {
                    s.error = Some(msg);
                }
                return;
            }
        };
        match self.runtime.block_on(
            self.handlers
                .create_organization_for_account(store, account_id, request),
        ) {
            Ok(resp) => {
                self.screen = Screen::Outcome(create_organization_outcome(&resp));
            }
            Err(reason) => {
                if let Screen::CreateOrganization(s) = &mut self.screen {
                    s.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_list_organizations(&mut self, store: &Store) {
        let account_id = match self.require_session_account_id() {
            Ok(id) => id,
            Err(msg) => {
                if let Screen::ListOrganizations(s) = &mut self.screen {
                    s.error = Some(msg);
                }
                return;
            }
        };
        match self
            .runtime
            .block_on(self.handlers.list_account_organizations(store, account_id))
        {
            Ok(orgs) => {
                let summary: Vec<_> = orgs
                    .iter()
                    .map(|o| (o.id.to_string(), o.name.to_string(), 0u64))
                    .collect();
                self.screen = Screen::Outcome(list_organizations_outcome(&summary));
            }
            Err(reason) => {
                if let Screen::ListOrganizations(s) = &mut self.screen {
                    s.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_org_admin_probe(&mut self, store: &Store) {
        let parsed = {
            let Screen::OrgAdminProbe(state) = &self.screen else {
                return;
            };
            parse_org_admin_probe(state)
        };
        let (org_id, operation) = match parsed {
            Ok(vals) => vals,
            Err(msg) => {
                if let Screen::OrgAdminProbe(s) = &mut self.screen {
                    s.error = Some(msg);
                }
                return;
            }
        };
        let account_id = match self.require_session_account_id() {
            Ok(id) => id,
            Err(msg) => {
                if let Screen::OrgAdminProbe(s) = &mut self.screen {
                    s.error = Some(msg);
                }
                return;
            }
        };
        match self.runtime.block_on(
            self.handlers
                .authorize_org_admin_operation(store, account_id, org_id, operation),
        ) {
            Ok(()) => {
                self.screen = Screen::Outcome(org_admin_probe_outcome(org_id, operation));
            }
            Err(reason) => {
                if let Screen::OrgAdminProbe(s) = &mut self.screen {
                    s.error = Some(render_error(reason));
                }
            }
        }
    }

    fn active_form_mut(&mut self) -> Option<&mut FormState> {
        match &mut self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::CreateOrganization(s)
            | Screen::ListOrganizations(s)
            | Screen::OrgAdminProbe(s) => Some(s),
            _ => None,
        }
    }

    fn require_session_account_id(&self) -> Result<AccountId, String> {
        self.session_account_id
            .ok_or_else(|| "auth_required: sign in first".to_owned())
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let area = frame.area();
        match &self.screen {
            Screen::Menu { selected } => draw::draw_menu(frame, area, *selected),
            Screen::SignUp(s) => draw::draw_form(frame, area, "Sign up", s),
            Screen::SignIn(s) => draw::draw_form(frame, area, "Sign in", s),
            Screen::AcceptInvitation(s) => draw::draw_form(frame, area, "Accept invitation", s),
            Screen::CreateOrganization(s) => {
                draw::draw_form(frame, area, "Create organization", s);
            }
            Screen::ListOrganizations(s) => {
                draw::draw_form(frame, area, "List organizations", s);
            }
            Screen::OrgAdminProbe(s) => {
                draw::draw_form(frame, area, "Authorize admin operation", s);
            }
            Screen::Outcome(view) => draw::draw_outcome(frame, area, view),
        }
    }
}
