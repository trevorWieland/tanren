//! TUI screen state machine and submit dispatch.
//!
//! Rendering in `draw.rs`, form factories + outcome adapters in `ui.rs`.

use std::env;
use std::io::Stdout;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tanren_app_services::{Handlers, Store};
use tanren_contract::{JoinOrganizationRequest, LeaveOrganizationRequest, RemoveMemberRequest};
use tanren_identity_policy::{AccountId, OrgId};
use tokio::runtime::Runtime;

use crate::draw;
use crate::ui::{
    accept_invitation_fields, accept_invitation_outcome, departure_outcome,
    join_organization_fields, join_organization_outcome, leave_organization_fields,
    parse_accept_invitation, parse_departure_leave, parse_departure_remove, parse_join_invitation,
    parse_sign_in, parse_sign_up, remove_member_fields, render_error, sign_in_fields,
    sign_in_outcome, sign_up_fields, sign_up_outcome, unauthenticated_error,
};
use crate::{DepartureKind, FormState, MenuChoice};

const DATABASE_URL_ENV: &str = "DATABASE_URL";
#[derive(Debug)]
pub(crate) enum Screen {
    Menu { selected: usize },
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
    JoinOrganization(FormState),
    Departure(DepartureKind, FormState),
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
    screen: Screen,
    account_id: Option<AccountId>,
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
            account_id: None,
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
            Screen::JoinOrganization(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::JoinOrganization),
                None => Effect::None,
            },
            Screen::Departure(dk, state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::Departure(*dk)),
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
                        self.account_id = Some(response.account.id);
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
                        self.account_id = Some(response.account.id);
                        self.screen = Screen::Outcome(sign_in_outcome(&response));
                    }
                    Err(reason) => {
                        if let Screen::SignIn(state) = &mut self.screen {
                            state.error = Some(render_error(reason));
                        }
                    }
                }
            }
            FormKind::AcceptInvitation => self.submit_accept_invitation(&store),
            FormKind::JoinOrganization => self.submit_join(&store),
            FormKind::Departure(dk) => self.submit_departure(dk, &store),
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
                self.account_id = Some(response.account.id);
                self.screen = Screen::Outcome(accept_invitation_outcome(&response));
            }
            Err(reason) => {
                if let Screen::AcceptInvitation(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_join(&mut self, store: &Arc<Store>) {
        let parsed = {
            let Screen::JoinOrganization(state) = &self.screen else {
                return;
            };
            parse_join_invitation(state)
        };
        let invitation_token = match parsed {
            Ok(token) => token,
            Err(message) => {
                if let Screen::JoinOrganization(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let Some(account_id) = self.account_id else {
            if let Screen::JoinOrganization(state) = &mut self.screen {
                state.error = Some("No active session. Sign in or sign up first.".to_owned());
            }
            return;
        };
        let request = JoinOrganizationRequest { invitation_token };
        let result = self.runtime.block_on(self.handlers.join_organization(
            store.as_ref(),
            account_id,
            request,
        ));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(join_organization_outcome(&response));
            }
            Err(reason) => {
                if let Screen::JoinOrganization(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_departure(&mut self, dk: DepartureKind, store: &Arc<Store>) {
        let Some(account_id) = self.account_id else {
            self.set_departure_error(&unauthenticated_error());
            return;
        };
        let parsed: Result<(OrgId, Option<AccountId>, bool), String> = {
            let Screen::Departure(_, st) = &self.screen else {
                return;
            };
            match dk {
                DepartureKind::Leave => parse_departure_leave(st).map(|(o, a)| (o, None, a)),
                DepartureKind::Remove => {
                    parse_departure_remove(st).map(|(o, t, a)| (o, Some(t), a))
                }
            }
        };
        let parsed = match parsed {
            Ok(p) => p,
            Err(e) => {
                self.set_departure_error(&e);
                return;
            }
        };
        let result = match parsed {
            (org_id, None, ack) => self.runtime.block_on(self.handlers.leave_organization(
                store.as_ref(),
                account_id,
                LeaveOrganizationRequest { org_id },
                ack,
            )),
            (org_id, Some(target), ack) => self.runtime.block_on(self.handlers.remove_member(
                store.as_ref(),
                account_id,
                RemoveMemberRequest {
                    org_id,
                    member_account_id: target,
                },
                ack,
            )),
        };
        match result {
            Ok(r) => self.screen = Screen::Outcome(departure_outcome(&r, dk)),
            Err(e) => self.set_departure_error(&render_error(e)),
        }
    }

    fn set_departure_error(&mut self, message: &str) {
        if let Screen::Departure(_, state) = &mut self.screen {
            state.error = Some(message.to_owned());
        }
    }

    fn active_form_mut(&mut self) -> Option<&mut FormState> {
        match &mut self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::JoinOrganization(s)
            | Screen::Departure(_, s) => Some(s),
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
            Screen::JoinOrganization(state) => {
                draw::draw_form(frame, area, "Join organization", state);
            }
            Screen::Departure(dk, state) => {
                let title = match dk {
                    DepartureKind::Leave => "Leave organization",
                    DepartureKind::Remove => "Remove member",
                };
                draw::draw_form(frame, area, title, state);
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
    JoinOrganization,
    Departure(DepartureKind),
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

fn handle_menu_key(selected: &mut usize, key: KeyEvent, next: &mut Option<Screen>) -> bool {
    match key.code {
        KeyCode::Char('q' | 'Q') | KeyCode::Esc => return true,
        KeyCode::Up => {
            if *selected == 0 {
                *selected = MenuChoice::ALL.len() - 1;
            } else {
                *selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Tab => {
            *selected = (*selected + 1) % MenuChoice::ALL.len();
        }
        KeyCode::Enter => {
            let choice = MenuChoice::ALL[*selected];
            *next = Some(match choice {
                MenuChoice::SignUp => Screen::SignUp(FormState::new(sign_up_fields())),
                MenuChoice::SignIn => Screen::SignIn(FormState::new(sign_in_fields())),
                MenuChoice::AcceptInvitation => {
                    Screen::AcceptInvitation(FormState::new(accept_invitation_fields()))
                }
                MenuChoice::JoinOrganization => {
                    Screen::JoinOrganization(FormState::new(join_organization_fields()))
                }
                MenuChoice::LeaveOrganization => Screen::Departure(
                    DepartureKind::Leave,
                    FormState::new(leave_organization_fields()),
                ),
                MenuChoice::RemoveMember => Screen::Departure(
                    DepartureKind::Remove,
                    FormState::new(remove_member_fields()),
                ),
            });
        }
        _ => {}
    }
    false
}

fn handle_form_key(state: &mut FormState, key: KeyEvent) -> Option<FormAction> {
    match key.code {
        KeyCode::Esc => Some(FormAction::Cancel),
        KeyCode::Enter => Some(FormAction::Submit),
        KeyCode::Tab | KeyCode::Down => {
            state.cycle_focus(true);
            None
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.cycle_focus(false);
            None
        }
        KeyCode::Backspace => {
            state.pop_char();
            None
        }
        KeyCode::Char(c) => {
            state.push_char(c);
            None
        }
        _ => None,
    }
}

fn is_press(key: &KeyEvent) -> bool {
    use crossterm::event::KeyEventKind;
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}
