//! TUI screen state machine and submit dispatch.
//!
//! Split out of `lib.rs` so the tui-app crate stays under the workspace
//! 500-line line-budget. Keeps the screen enum, the `App` struct, and
//! the form/menu key handlers together; rendering still lives in
//! `draw.rs`, form factories + outcome adapters in `ui.rs`.

mod input;

use std::env;
use std::io::Stdout;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tanren_app_services::{AppServiceError, Handlers, Store};
use tanren_contract::{AccountFailureReason, SessionView};
use tokio::runtime::Runtime;

use crate::FormState;
use crate::draw;
use crate::ui::{
    accept_invitation_outcome, auth_required_message, check_organization_permission_outcome,
    create_organization_outcome, list_organizations_outcome, parse_accept_invitation,
    parse_check_organization_permission, parse_create_organization, parse_list_organizations,
    parse_sign_in, parse_sign_up, render_error, sign_in_outcome, sign_up_outcome,
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
    CheckOrganizationPermission(FormState),
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
    active_session: Option<SessionView>,
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
            active_session: None,
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
            if !input::is_press(&key) {
                continue;
            }
            if self.handle_key(key) {
                return Ok(());
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            return true;
        }
        let effect = match &mut self.screen {
            Screen::Menu { selected } => {
                let mut next: Option<Screen> = None;
                let exit = input::handle_menu_key(selected, key, &mut next);
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
                    Effect::ReplaceScreen(Screen::Menu { selected: 0 })
                } else {
                    Effect::None
                }
            }
            Screen::SignUp(state) => match input::handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::SignUp),
                None => Effect::None,
            },
            Screen::SignIn(state) => match input::handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::SignIn),
                None => Effect::None,
            },
            Screen::AcceptInvitation(state) => match input::handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::AcceptInvitation),
                None => Effect::None,
            },
            Screen::CreateOrganization(state) => match input::handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::CreateOrganization),
                None => Effect::None,
            },
            Screen::ListOrganizations(state) => match input::handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::ListOrganizations),
                None => Effect::None,
            },
            Screen::CheckOrganizationPermission(state) => {
                match input::handle_form_key(state, key) {
                    Some(action) => Effect::Form(action, FormKind::CheckOrganizationPermission),
                    None => Effect::None,
                }
            }
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
            FormKind::SignUp => self.submit_sign_up(store.as_ref()),
            FormKind::SignIn => self.submit_sign_in(store.as_ref()),
            FormKind::AcceptInvitation => self.submit_accept_invitation(store.as_ref()),
            FormKind::CreateOrganization => self.submit_create_organization(store.as_ref()),
            FormKind::ListOrganizations => self.submit_list_organizations(store.as_ref()),
            FormKind::CheckOrganizationPermission => {
                self.submit_check_organization_permission(store.as_ref());
            }
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
            Err(message) => {
                if let Screen::SignUp(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self.runtime.block_on(self.handlers.sign_up(store, request));
        match result {
            Ok(response) => {
                self.active_session = Some(response.session.clone());
                self.screen = Screen::Outcome(sign_up_outcome(&response));
            }
            Err(reason) => {
                if let Screen::SignUp(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
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
            Err(message) => {
                if let Screen::SignIn(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self.runtime.block_on(self.handlers.sign_in(store, request));
        match result {
            Ok(response) => {
                self.active_session = Some(response.session.clone());
                self.screen = Screen::Outcome(sign_in_outcome(&response));
            }
            Err(reason) => {
                if let Screen::SignIn(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
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
            Err(message) => {
                if let Screen::AcceptInvitation(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.accept_invitation(store, request));
        match result {
            Ok(response) => {
                self.active_session = Some(response.session.clone());
                self.screen = Screen::Outcome(accept_invitation_outcome(&response));
            }
            Err(reason) => {
                if let Screen::AcceptInvitation(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_create_organization(&mut self, store: &Store) {
        let Some(session) = self.active_session.clone() else {
            if let Screen::CreateOrganization(state) = &mut self.screen {
                state.error = Some(auth_required_message());
            }
            return;
        };
        let parsed = {
            let Screen::CreateOrganization(state) = &self.screen else {
                return;
            };
            parse_create_organization(state, &session)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(message) => {
                if let Screen::CreateOrganization(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.create_organization(store, request));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(create_organization_outcome(&response));
            }
            Err(reason) => {
                if let Screen::CreateOrganization(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_list_organizations(&mut self, store: &Store) {
        let Some(session) = self.active_session.clone() else {
            if let Screen::ListOrganizations(state) = &mut self.screen {
                state.error = Some(auth_required_message());
            }
            return;
        };
        let request = parse_list_organizations(&session);
        let result = self
            .runtime
            .block_on(self.handlers.list_organizations(store, request));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(list_organizations_outcome(&response));
            }
            Err(reason) => {
                if let Screen::ListOrganizations(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_check_organization_permission(&mut self, store: &Store) {
        let Some(session) = self.active_session.clone() else {
            if let Screen::CheckOrganizationPermission(state) = &mut self.screen {
                state.error = Some(auth_required_message());
            }
            return;
        };
        let parsed = {
            let Screen::CheckOrganizationPermission(state) = &self.screen else {
                return;
            };
            parse_check_organization_permission(state, &session)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(message) => {
                if let Screen::CheckOrganizationPermission(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.check_organization_permission(store, request));
        match result {
            Ok(response) if response.allowed => {
                self.screen = Screen::Outcome(check_organization_permission_outcome(&response));
            }
            Ok(_) => {
                if let Screen::CheckOrganizationPermission(state) = &mut self.screen {
                    state.error = Some(render_error(AppServiceError::Account(
                        AccountFailureReason::PermissionDenied,
                    )));
                }
            }
            Err(reason) => {
                if let Screen::CheckOrganizationPermission(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
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
            | Screen::CheckOrganizationPermission(s) => Some(s),
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
            Screen::CreateOrganization(state) => {
                draw::draw_form(frame, area, "Create organization", state);
            }
            Screen::ListOrganizations(state) => {
                draw::draw_form(frame, area, "List organizations", state);
            }
            Screen::CheckOrganizationPermission(state) => {
                draw::draw_form(frame, area, "Check org permission", state);
            }
            Screen::Outcome(view) => draw::draw_outcome(frame, area, view),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FormKind {
    SignUp,
    SignIn,
    AcceptInvitation,
    CreateOrganization,
    ListOrganizations,
    CheckOrganizationPermission,
}

#[derive(Debug, Clone, Copy)]
enum FormAction {
    Submit,
    Cancel,
}

#[derive(Debug)]
enum Effect {
    None,
    Exit,
    ReplaceScreen(Screen),
    Form(FormAction, FormKind),
}
