//! TUI screen state machine and submit dispatch. Split out of `lib.rs`
//! so the crate stays under the workspace 500-line line-budget. Keeps
//! the screen enum, the `App` struct, and the form/menu key handlers
//! together; rendering still lives in `draw.rs`, form factories +
//! outcome adapters in `ui.rs`.

use std::env;
use std::io::Stdout;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tanren_app_services::{Handlers, Store};
use tokio::runtime::Runtime;

use crate::draw;
use crate::ui::{
    accept_invitation_fields, accept_invitation_outcome, handle_form_key, is_press,
    list_org_projects_outcome, list_orgs_fields, list_orgs_outcome, parse_accept_invitation,
    parse_account_and_org_id, parse_account_id, parse_sign_in, parse_sign_up, render_error,
    sign_in_fields, sign_in_outcome, sign_up_fields, sign_up_outcome, switch_org_fields,
    switch_org_outcome,
};
use crate::{FormAction, FormState, MenuChoice};

const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Debug)]
pub(crate) enum Screen {
    Menu { selected: usize },
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
    ListOrgs(FormState),
    SwitchOrg(FormState),
    OrgProjects(FormState),
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
    has_orgs: Option<bool>,
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
            has_orgs: None,
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
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            return true;
        }
        let effect = match &mut self.screen {
            Screen::Menu { selected } => {
                let mut next: Option<Screen> = None;
                let exit = handle_menu_key(selected, key, &mut next, self.has_orgs);
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
            Screen::ListOrgs(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::ListOrgs),
                None => Effect::None,
            },
            Screen::SwitchOrg(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::SwitchOrg),
                None => Effect::None,
            },
            Screen::OrgProjects(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::OrgProjects),
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
            FormKind::ListOrgs => self.submit_list_orgs(&store),
            FormKind::SwitchOrg => self.submit_switch_org(&store),
            FormKind::OrgProjects => self.submit_org_projects(&store),
        }
    }

    fn submit_sign_up(&mut self, store: &Arc<Store>) {
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
            .block_on(self.handlers.sign_up(store.as_ref(), request));
        match result {
            Ok(response) => self.screen = Screen::Outcome(sign_up_outcome(&response)),
            Err(reason) => {
                if let Screen::SignUp(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_sign_in(&mut self, store: &Arc<Store>) {
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
            .block_on(self.handlers.sign_in(store.as_ref(), request));
        match result {
            Ok(response) => self.screen = Screen::Outcome(sign_in_outcome(&response)),
            Err(reason) => {
                if let Screen::SignIn(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_accept_invitation(&mut self, store: &Arc<Store>) {
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
            .block_on(self.handlers.accept_invitation(store.as_ref(), request));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(accept_invitation_outcome(&response));
            }
            Err(reason) => {
                if let Screen::AcceptInvitation(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_list_orgs(&mut self, store: &Arc<Store>) {
        let parsed = {
            let Screen::ListOrgs(state) = &self.screen else {
                return;
            };
            parse_account_id(state)
        };
        let account_id = match parsed {
            Ok(id) => id,
            Err(message) => {
                if let Screen::ListOrgs(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.list_organizations(store.as_ref(), account_id));
        match result {
            Ok(response) => {
                self.has_orgs = Some(!response.memberships.is_empty());
                self.screen = Screen::Outcome(list_orgs_outcome(&response));
            }
            Err(reason) => {
                if let Screen::ListOrgs(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_switch_org(&mut self, store: &Arc<Store>) {
        let parsed = {
            let Screen::SwitchOrg(state) = &self.screen else {
                return;
            };
            parse_account_and_org_id(state)
        };
        let (account_id, request) = match parsed {
            Ok(pair) => pair,
            Err(message) => {
                if let Screen::SwitchOrg(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self.runtime.block_on(self.handlers.switch_active_org(
            store.as_ref(),
            account_id,
            request,
        ));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(switch_org_outcome(&response));
            }
            Err(reason) => {
                if let Screen::SwitchOrg(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_org_projects(&mut self, store: &Arc<Store>) {
        let parsed = {
            let Screen::OrgProjects(state) = &self.screen else {
                return;
            };
            parse_account_id(state)
        };
        let account_id = match parsed {
            Ok(id) => id,
            Err(message) => {
                if let Screen::OrgProjects(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self.runtime.block_on(
            self.handlers
                .list_active_org_projects(store.as_ref(), account_id),
        );
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(list_org_projects_outcome(&response));
            }
            Err(reason) => {
                if let Screen::OrgProjects(state) = &mut self.screen {
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
            | Screen::ListOrgs(s)
            | Screen::SwitchOrg(s)
            | Screen::OrgProjects(s) => Some(s),
            _ => None,
        }
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let area = frame.area();
        match &self.screen {
            Screen::Menu { selected } => draw::draw_menu(frame, area, *selected, self.has_orgs),
            Screen::SignUp(state) => draw::draw_form(frame, area, "Sign up", state),
            Screen::SignIn(state) => draw::draw_form(frame, area, "Sign in", state),
            Screen::AcceptInvitation(state) => {
                draw::draw_form(frame, area, "Accept invitation", state);
            }
            Screen::ListOrgs(state) => draw::draw_form(frame, area, "List organizations", state),
            Screen::SwitchOrg(state) => draw::draw_form(frame, area, "Switch active org", state),
            Screen::OrgProjects(state) => draw::draw_form(frame, area, "Org projects", state),
            Screen::Outcome(view) => draw::draw_outcome(frame, area, view),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FormKind {
    SignUp,
    SignIn,
    AcceptInvitation,
    ListOrgs,
    SwitchOrg,
    OrgProjects,
}

#[derive(Debug)]
enum Effect {
    None,
    Exit,
    ReplaceScreen(Screen),
    Form(FormAction, FormKind),
}

fn handle_menu_key(
    selected: &mut usize,
    key: KeyEvent,
    next: &mut Option<Screen>,
    has_orgs: Option<bool>,
) -> bool {
    let choices = MenuChoice::available(has_orgs);
    match key.code {
        KeyCode::Char('q' | 'Q') | KeyCode::Esc => return true,
        KeyCode::Up => {
            if *selected == 0 {
                *selected = choices.len() - 1;
            } else {
                *selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Tab => {
            *selected = (*selected + 1) % choices.len();
        }
        KeyCode::Enter => {
            let choice = choices[*selected];
            *next = Some(match choice {
                MenuChoice::SignUp => Screen::SignUp(FormState::new(sign_up_fields())),
                MenuChoice::SignIn => Screen::SignIn(FormState::new(sign_in_fields())),
                MenuChoice::AcceptInvitation => {
                    Screen::AcceptInvitation(FormState::new(accept_invitation_fields()))
                }
                MenuChoice::ListOrganizations => {
                    Screen::ListOrgs(FormState::new(list_orgs_fields()))
                }
                MenuChoice::SwitchActiveOrg => {
                    Screen::SwitchOrg(FormState::new(switch_org_fields()))
                }
                MenuChoice::ListOrgProjects => {
                    Screen::OrgProjects(FormState::new(list_orgs_fields()))
                }
            });
        }
        _ => {}
    }
    false
}
