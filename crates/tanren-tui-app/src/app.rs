//! TUI screen state machine; key handlers in `app_input.rs`, rendering in `draw.rs`.
use std::env;
use std::io::Stdout;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tanren_app_services::{Handlers, Store};
use tanren_contract::OrganizationView;
use tanren_identity_policy::SessionToken;
use tokio::runtime::Runtime;

use crate::FormState;
use crate::app_input::{
    DashboardOutcome, Effect, FormAction, FormKind, handle_dashboard_key, handle_form_key,
    handle_menu_key, is_press,
};
use crate::draw;
use crate::ui::{
    accept_invitation_outcome, parse_accept_invitation, parse_org_create, parse_sign_in,
    parse_sign_up, render_error, sign_in_outcome, sign_up_outcome,
};

const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Debug)]
pub(crate) enum Screen {
    Menu {
        selected: usize,
    },
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
    Outcome(OutcomeView),
    Dashboard {
        selected: usize,
    },
    OrgCreate(FormState),
    OrgList {
        orgs: Vec<OrganizationView>,
        selected: usize,
        error: Option<String>,
    },
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
    screen: Screen,
    session_token: Option<SessionToken>,
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
            screen: Screen::Menu { selected: 0 },
            session_token: None,
        })
    }

    pub(crate) fn with_store(store: Arc<Store>) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("build tokio runtime")?;
        Ok(Self {
            runtime,
            handlers: Handlers::new(),
            store: Some(store),
            store_error: None,
            screen: Screen::Menu { selected: 0 },
            session_token: None,
        })
    }

    pub(crate) fn screen_kind(&self) -> crate::harness::ScreenKind {
        match &self.screen {
            Screen::Menu { .. } => crate::harness::ScreenKind::Menu,
            Screen::SignUp(_) => crate::harness::ScreenKind::SignUp,
            Screen::SignIn(_) => crate::harness::ScreenKind::SignIn,
            Screen::AcceptInvitation(_) => crate::harness::ScreenKind::AcceptInvitation,
            Screen::Outcome(_) => crate::harness::ScreenKind::Outcome,
            Screen::Dashboard { .. } => crate::harness::ScreenKind::Dashboard,
            Screen::OrgCreate(_) => crate::harness::ScreenKind::OrgCreate,
            Screen::OrgList { .. } => crate::harness::ScreenKind::OrgList,
        }
    }

    pub(crate) fn screen_banner(&self) -> Option<String> {
        match &self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::OrgCreate(s) => s.error.clone(),
            Screen::OrgList { error, .. } => error.clone(),
            _ => None,
        }
    }
    pub(crate) fn set_session_token(&mut self, token: SessionToken) {
        self.session_token = Some(token);
    }
    pub(crate) fn session_token_ref(&self) -> Option<&SessionToken> {
        self.session_token.as_ref()
    }
    #[cfg(any(test, feature = "test-hooks"))]
    pub(crate) fn navigate_to_dashboard(&mut self) {
        self.screen = Screen::Dashboard { selected: 0 };
    }
    #[cfg(any(test, feature = "test-hooks"))]
    pub(crate) fn org_list_data(&self) -> Vec<OrganizationView> {
        match &self.screen {
            Screen::OrgList { orgs, .. } => orgs.clone(),
            _ => Vec::new(),
        }
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

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            return true;
        }
        let effect = match &mut self.screen {
            Screen::Menu { selected } => {
                let mut next: Option<Screen> = None;
                let exit = handle_menu_key(selected, key, &mut next);
                if exit {
                    Effect::Exit
                } else if let Some(screen) = next {
                    Effect::ReplaceScreen(screen)
                } else {
                    Effect::None
                }
            }
            Screen::Outcome(_) => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | 'Q')
                ) {
                    if self.session_token.is_some() {
                        Effect::ReplaceScreen(Screen::Dashboard { selected: 0 })
                    } else {
                        Effect::ReplaceScreen(Screen::Menu { selected: 0 })
                    }
                } else {
                    Effect::None
                }
            }
            Screen::Dashboard { selected } => {
                let session_present = self.session_token.is_some();
                match handle_dashboard_key(selected, key, session_present) {
                    DashboardOutcome::None => Effect::None,
                    DashboardOutcome::Exit => Effect::Exit,
                    DashboardOutcome::Screen(s) => Effect::ReplaceScreen(s),
                    DashboardOutcome::NavigateToOrgList => Effect::NavigateToOrgList,
                }
            }
            Screen::OrgCreate(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::OrgCreate),
                None => Effect::None,
            },
            Screen::OrgList {
                orgs,
                selected,
                error,
            } => match key.code {
                KeyCode::Esc => Effect::ReplaceScreen(Screen::Dashboard { selected: 0 }),
                KeyCode::Char('q' | 'Q') => {
                    Effect::ReplaceScreen(Screen::Dashboard { selected: 0 })
                }
                KeyCode::Up => {
                    if !orgs.is_empty() {
                        *selected = if *selected == 0 {
                            orgs.len() - 1
                        } else {
                            *selected - 1
                        };
                    }
                    Effect::None
                }
                KeyCode::Down => {
                    if !orgs.is_empty() {
                        *selected = (*selected + 1) % orgs.len();
                    }
                    Effect::None
                }
                KeyCode::Char('r') => {
                    let _ = error;
                    Effect::NavigateToOrgList
                }
                _ => Effect::None,
            },
            Screen::SignUp(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::SignUp),
                None => Effect::None,
            },
            Screen::SignIn(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::SignIn),
                None => Effect::None,
            },
            Screen::AcceptInvitation(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::AcceptInvitation),
                None => Effect::None,
            },
        };
        match effect {
            Effect::None => false,
            Effect::Exit => true,
            Effect::ReplaceScreen(screen) => {
                self.screen = screen;
                false
            }
            Effect::Form(action, kind) => {
                self.dispatch_form_action(action, kind);
                false
            }
            Effect::NavigateToOrgList => {
                self.navigate_to_org_list();
                false
            }
        }
    }

    fn dispatch_form_action(&mut self, action: FormAction, kind: FormKind) {
        match action {
            FormAction::Cancel => {
                if self.session_token.is_some() {
                    self.screen = Screen::Dashboard { selected: 0 };
                } else {
                    self.screen = Screen::Menu { selected: 0 };
                }
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
        let handlers = &self.handlers;
        match kind {
            FormKind::SignUp => {
                let parsed = {
                    let Screen::SignUp(state) = &self.screen else {
                        return;
                    };
                    parse_sign_up(state)
                };
                let request = match parsed {
                    Ok(req) => req,
                    Err(message) => {
                        if let Screen::SignUp(state) = &mut self.screen {
                            state.error = Some(message);
                        }
                        return;
                    }
                };
                let result = self
                    .runtime
                    .block_on(handlers.sign_up(store.as_ref(), request));
                match result {
                    Ok(response) => {
                        self.session_token = Some(response.session.token.clone());
                        self.screen = Screen::Outcome(sign_up_outcome(&response));
                    }
                    Err(reason) => {
                        if let Screen::SignUp(state) = &mut self.screen {
                            state.error = Some(render_error(reason));
                        }
                    }
                }
            }
            FormKind::SignIn => {
                let parsed = {
                    let Screen::SignIn(state) = &self.screen else {
                        return;
                    };
                    parse_sign_in(state)
                };
                let request = match parsed {
                    Ok(req) => req,
                    Err(message) => {
                        if let Screen::SignIn(state) = &mut self.screen {
                            state.error = Some(message);
                        }
                        return;
                    }
                };
                let result = self
                    .runtime
                    .block_on(handlers.sign_in(store.as_ref(), request));
                match result {
                    Ok(response) => {
                        self.session_token = Some(response.session.token.clone());
                        self.screen = Screen::Outcome(sign_in_outcome(&response));
                    }
                    Err(reason) => {
                        if let Screen::SignIn(state) = &mut self.screen {
                            state.error = Some(render_error(reason));
                        }
                    }
                }
            }
            FormKind::AcceptInvitation => {
                let parsed = {
                    let Screen::AcceptInvitation(state) = &self.screen else {
                        return;
                    };
                    parse_accept_invitation(state)
                };
                let request = match parsed {
                    Ok(req) => req,
                    Err(message) => {
                        if let Screen::AcceptInvitation(state) = &mut self.screen {
                            state.error = Some(message);
                        }
                        return;
                    }
                };
                let result = self
                    .runtime
                    .block_on(handlers.accept_invitation(store.as_ref(), request));
                match result {
                    Ok(response) => {
                        self.session_token = Some(response.session.token.clone());
                        self.screen = Screen::Outcome(accept_invitation_outcome(&response));
                    }
                    Err(reason) => {
                        if let Screen::AcceptInvitation(state) = &mut self.screen {
                            state.error = Some(render_error(reason));
                        }
                    }
                }
            }
            FormKind::OrgCreate => {
                let Some(session) = self.session_token.clone() else {
                    if let Screen::OrgCreate(state) = &mut self.screen {
                        state.error = Some("not authenticated".to_owned());
                    }
                    return;
                };
                let parsed = {
                    let Screen::OrgCreate(state) = &self.screen else {
                        return;
                    };
                    parse_org_create(state)
                };
                let request = match parsed {
                    Ok(req) => req,
                    Err(message) => {
                        if let Screen::OrgCreate(state) = &mut self.screen {
                            state.error = Some(message);
                        }
                        return;
                    }
                };
                let result = self.runtime.block_on(handlers.create_organization(
                    store.as_ref(),
                    &session,
                    request,
                ));
                match result {
                    Ok(_) => {
                        self.navigate_to_org_list();
                    }
                    Err(reason) => {
                        if let Screen::OrgCreate(state) = &mut self.screen {
                            state.error = Some(render_error(reason));
                        }
                    }
                }
            }
        }
    }

    fn navigate_to_org_list(&mut self) {
        let Some(store) = self.store.clone() else {
            self.screen = Screen::OrgList {
                orgs: Vec::new(),
                selected: 0,
                error: self
                    .store_error
                    .clone()
                    .or_else(|| Some("store unavailable".to_owned())),
            };
            return;
        };
        let Some(session) = self.session_token.clone() else {
            self.screen = Screen::OrgList {
                orgs: Vec::new(),
                selected: 0,
                error: Some("not authenticated".to_owned()),
            };
            return;
        };
        let result = self
            .runtime
            .block_on(self.handlers.list_organizations(store.as_ref(), &session));
        self.screen = match result {
            Ok(response) => Screen::OrgList {
                orgs: response.organizations,
                selected: 0,
                error: None,
            },
            Err(reason) => Screen::OrgList {
                orgs: Vec::new(),
                selected: 0,
                error: Some(render_error(reason)),
            },
        };
    }

    fn active_form_mut(&mut self) -> Option<&mut FormState> {
        match &mut self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::OrgCreate(s) => Some(s),
            _ => None,
        }
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let area = frame.area();
        match &self.screen {
            Screen::Menu { selected } => draw::draw_menu(frame, area, *selected),
            Screen::SignUp(state) => draw::draw_form(frame, area, "Sign up", state),
            Screen::SignIn(state) => draw::draw_form(frame, area, "Sign in", state),
            Screen::AcceptInvitation(state) => {
                draw::draw_form(frame, area, "Accept invitation", state);
            }
            Screen::Outcome(view) => draw::draw_outcome(frame, area, view),
            Screen::Dashboard { selected } => draw::draw_dashboard(frame, area, *selected),
            Screen::OrgCreate(state) => draw::draw_form(frame, area, "Create organization", state),
            Screen::OrgList {
                orgs,
                selected,
                error,
            } => draw::draw_org_list(frame, area, orgs, *selected, error.as_deref()),
        }
    }
}
