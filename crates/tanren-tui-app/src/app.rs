//! TUI screen state machine and submit dispatch.
//!
//! Split out of `lib.rs` to keep this crate under the 500-line budget.
//! Rendering lives in `draw.rs`; form factories + outcome adapters in `ui.rs`.

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
    accept_invitation_fields, accept_invitation_outcome, create_invitation_fields,
    create_invitation_outcome, list_invitations_fields, list_invitations_outcome,
    parse_accept_invitation, parse_create_invitation, parse_list_invitation_inputs,
    parse_revoke_invitation, parse_sign_in, parse_sign_up, render_error, revoke_invitation_fields,
    revoke_invitation_outcome, sign_in_fields, sign_in_outcome, sign_up_fields, sign_up_outcome,
};
use crate::{FormState, MenuChoice};

const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Debug)]
pub(crate) enum Screen {
    Menu { selected: usize },
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
    CreateInvitation(FormState),
    ListInvitations(FormState),
    RevokeInvitation(FormState),
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
            Screen::SignUp(state) => form_effect(handle_form_key(state, key), FormKind::SignUp),
            Screen::SignIn(state) => form_effect(handle_form_key(state, key), FormKind::SignIn),
            Screen::AcceptInvitation(state) => {
                form_effect(handle_form_key(state, key), FormKind::AcceptInvitation)
            }
            Screen::CreateInvitation(state) => {
                form_effect(handle_form_key(state, key), FormKind::CreateInvitation)
            }
            Screen::ListInvitations(state) => {
                form_effect(handle_form_key(state, key), FormKind::ListInvitations)
            }
            Screen::RevokeInvitation(state) => {
                form_effect(handle_form_key(state, key), FormKind::RevokeInvitation)
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
        let Some(arc) = self.store.clone() else {
            let message = self
                .store_error
                .clone()
                .unwrap_or_else(|| "store unavailable".to_owned());
            if let Some(state) = self.active_form_mut() {
                state.error = Some(message);
            }
            return;
        };
        let store = arc.as_ref();
        match kind {
            FormKind::SignUp => self.submit_sign_up(store),
            FormKind::SignIn => self.submit_sign_in(store),
            FormKind::AcceptInvitation => self.submit_accept_invitation(store),
            FormKind::CreateInvitation => self.submit_create_invitation(store),
            FormKind::ListInvitations => self.submit_list_invitations(store),
            FormKind::RevokeInvitation => self.submit_revoke_invitation(store),
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
        match self.runtime.block_on(self.handlers.sign_up(store, request)) {
            Ok(response) => self.screen = Screen::Outcome(sign_up_outcome(&response)),
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
        match self.runtime.block_on(self.handlers.sign_in(store, request)) {
            Ok(response) => self.screen = Screen::Outcome(sign_in_outcome(&response)),
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
        match self
            .runtime
            .block_on(self.handlers.accept_invitation(store, request))
        {
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

    fn submit_create_invitation(&mut self, store: &Store) {
        let parsed = {
            let Screen::CreateInvitation(state) = &self.screen else {
                return;
            };
            parse_create_invitation(state)
        };
        let inputs = match parsed {
            Ok(inp) => inp,
            Err(message) => {
                if let Screen::CreateInvitation(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        match self.runtime.block_on(self.handlers.create_invitation(
            store,
            inputs.caller_account_id,
            inputs.caller_org_context,
            inputs.request,
        )) {
            Ok(response) => {
                self.screen = Screen::Outcome(create_invitation_outcome(&response));
            }
            Err(reason) => {
                if let Screen::CreateInvitation(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_list_invitations(&mut self, store: &Store) {
        let parsed = {
            let Screen::ListInvitations(state) = &self.screen else {
                return;
            };
            parse_list_invitation_inputs(state)
        };
        let inputs = match parsed {
            Ok(inp) => inp,
            Err(message) => {
                if let Screen::ListInvitations(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        match self.runtime.block_on(self.handlers.list_org_invitations(
            store,
            inputs.caller_account_id,
            inputs.org_id,
        )) {
            Ok(response) => {
                self.screen = Screen::Outcome(list_invitations_outcome(&response));
            }
            Err(reason) => {
                if let Screen::ListInvitations(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_revoke_invitation(&mut self, store: &Store) {
        let parsed = {
            let Screen::RevokeInvitation(state) = &self.screen else {
                return;
            };
            parse_revoke_invitation(state)
        };
        let inputs = match parsed {
            Ok(inp) => inp,
            Err(message) => {
                if let Screen::RevokeInvitation(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        match self.runtime.block_on(self.handlers.revoke_invitation(
            store,
            inputs.caller_account_id,
            inputs.caller_org_context,
            inputs.request,
        )) {
            Ok(response) => {
                self.screen = Screen::Outcome(revoke_invitation_outcome(&response));
            }
            Err(reason) => {
                if let Screen::RevokeInvitation(state) = &mut self.screen {
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
            | Screen::CreateInvitation(s)
            | Screen::ListInvitations(s)
            | Screen::RevokeInvitation(s) => Some(s),
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
            Screen::CreateInvitation(state) => {
                draw::draw_form(frame, area, "Create invitation", state);
            }
            Screen::ListInvitations(state) => {
                draw::draw_form(frame, area, "List invitations", state);
            }
            Screen::RevokeInvitation(state) => {
                draw::draw_form(frame, area, "Revoke invitation", state);
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
    CreateInvitation,
    ListInvitations,
    RevokeInvitation,
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
                MenuChoice::CreateInvitation => {
                    Screen::CreateInvitation(FormState::new(create_invitation_fields()))
                }
                MenuChoice::ListInvitations => {
                    Screen::ListInvitations(FormState::new(list_invitations_fields()))
                }
                MenuChoice::RevokeInvitation => {
                    Screen::RevokeInvitation(FormState::new(revoke_invitation_fields()))
                }
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

fn form_effect(action: Option<FormAction>, kind: FormKind) -> Effect {
    match action {
        Some(a) => Effect::Form(a, kind),
        None => Effect::None,
    }
}
