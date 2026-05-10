//! TUI screen state machine and submit dispatch.

use std::env;
use std::io::Stdout;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tanren_app_services::{AccountErrorProjection, Clock, Handlers, Store};
use tanren_contract::{ListActiveAccountsRequest, SignedInAccountView, SwitchActiveAccountRequest};
use tokio::runtime::Runtime;

use crate::draw;
use crate::session::{active_context_from_session, set_active_account};
use crate::ui::{
    accept_invitation_fields, active_accounts_outcome, render_error, sign_in_fields,
    sign_up_fields, switch_active_outcome,
};
use crate::{FormState, MenuChoice};
mod input;
mod submit;
use self::input::{
    Effect, FormAction, FormKind, handle_form_key, handle_menu_key, handle_switch_active_key,
    is_press,
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
    SwitchActive {
        accounts: Vec<SignedInAccountView>,
        selected: usize,
        error: Option<String>,
    },
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
    clock: Clock,
    handlers: Handlers,
    store: Option<Arc<Store>>,
    store_error: Option<String>,
    screen: Screen,
}

impl App {
    pub(crate) fn new() -> Result<Self> {
        let clock = Clock::default();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("build tokio runtime")?;
        let (store, store_error) = match env::var(DATABASE_URL_ENV) {
            Ok(url) if !url.is_empty() => match runtime.block_on(Store::connect(&url)) {
                Ok(store) => (Some(Arc::new(store)), None),
                Err(err) => {
                    tracing::error!(target: "tanren_tui", error = %err, "connect to store");
                    let projected = AccountErrorProjection::internal();
                    (
                        None,
                        Some(format!("{}: {}", projected.code, projected.summary)),
                    )
                }
            },
            _ => (
                None,
                Some(format!("{DATABASE_URL_ENV} is not set; submit will fail.")),
            ),
        };
        Ok(Self {
            runtime,
            handlers: Handlers::with_clock(clock.clone()),
            clock,
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
                let mut choice: Option<MenuChoice> = None;
                let exit = handle_menu_key(selected, key, &mut choice);
                if exit {
                    Effect::Exit
                } else if let Some(choice) = choice {
                    Effect::Menu(choice)
                } else {
                    Effect::None
                }
            }
            Screen::SwitchActive {
                accounts,
                selected,
                error,
            } => handle_switch_active_key(accounts, selected, error, key),
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
        };
        match effect {
            Effect::None => false,
            Effect::Exit => true,
            Effect::ReplaceScreen(screen) => {
                self.screen = screen;
                false
            }
            Effect::Menu(choice) => {
                self.select_menu_choice(choice);
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
            FormAction::Submit => match kind {
                FormKind::SwitchActive => self.submit_switch_active(),
                _ => self.submit(kind),
            },
        }
    }

    fn select_menu_choice(&mut self, choice: MenuChoice) {
        match choice {
            MenuChoice::SignUp => self.screen = Screen::SignUp(FormState::new(sign_up_fields())),
            MenuChoice::SignIn => self.screen = Screen::SignIn(FormState::new(sign_in_fields())),
            MenuChoice::AcceptInvitation => {
                self.screen = Screen::AcceptInvitation(FormState::new(accept_invitation_fields()));
            }
            MenuChoice::ListActiveAccounts => self.list_active_accounts(),
            MenuChoice::SwitchActiveAccount => self.open_switch_active_screen(),
        }
    }

    fn list_active_accounts(&mut self) {
        let Some(store) = self.store.clone() else {
            self.screen = Screen::Outcome(OutcomeView {
                title: "List signed-in accounts failed",
                lines: vec![
                    self.store_error
                        .clone()
                        .unwrap_or_else(|| "store unavailable".to_owned()),
                ],
            });
            return;
        };
        let context = match self
            .runtime
            .block_on(active_context_from_session(store.as_ref(), &self.clock))
        {
            Ok(context) => context,
            Err(err) => {
                self.screen = Screen::Outcome(OutcomeView {
                    title: "List signed-in accounts failed",
                    lines: vec![err.to_string()],
                });
                return;
            }
        };
        match self.runtime.block_on(self.handlers.list_active_accounts(
            store.as_ref(),
            &context,
            ListActiveAccountsRequest::default(),
        )) {
            Ok(response) => {
                self.screen = Screen::Outcome(active_accounts_outcome(&response.accounts));
            }
            Err(reason) => {
                self.screen = Screen::Outcome(OutcomeView {
                    title: "List signed-in accounts failed",
                    lines: vec![render_error(&reason)],
                });
            }
        }
    }

    fn open_switch_active_screen(&mut self) {
        let Some(store) = self.store.clone() else {
            self.screen = Screen::Outcome(OutcomeView {
                title: "Switch active account failed",
                lines: vec![
                    self.store_error
                        .clone()
                        .unwrap_or_else(|| "store unavailable".to_owned()),
                ],
            });
            return;
        };
        let context = match self
            .runtime
            .block_on(active_context_from_session(store.as_ref(), &self.clock))
        {
            Ok(context) => context,
            Err(err) => {
                self.screen = Screen::Outcome(OutcomeView {
                    title: "Switch active account failed",
                    lines: vec![err.to_string()],
                });
                return;
            }
        };
        match self.runtime.block_on(self.handlers.list_active_accounts(
            store.as_ref(),
            &context,
            ListActiveAccountsRequest::default(),
        )) {
            Ok(response) => {
                let selected = response
                    .accounts
                    .iter()
                    .position(|entry| entry.is_active)
                    .unwrap_or(0);
                self.screen = Screen::SwitchActive {
                    accounts: response.accounts,
                    selected,
                    error: None,
                };
            }
            Err(reason) => {
                self.screen = Screen::Outcome(OutcomeView {
                    title: "Switch active account failed",
                    lines: vec![render_error(&reason)],
                });
            }
        }
    }

    fn submit_switch_active(&mut self) {
        let target_account_id = match &self.screen {
            Screen::SwitchActive {
                accounts, selected, ..
            } => {
                if let Ok(raw) = env::var("TANREN_TUI_TEST_SWITCH_TARGET_ACCOUNT_ID") {
                    if let Ok(parsed) = uuid::Uuid::parse_str(&raw) {
                        tanren_identity_policy::AccountId::new(parsed)
                    } else {
                        let Some(entry) = accounts.get(*selected) else {
                            return;
                        };
                        entry.account.id
                    }
                } else {
                    let Some(entry) = accounts.get(*selected) else {
                        return;
                    };
                    entry.account.id
                }
            }
            _ => return,
        };

        let Some(store) = self.store.clone() else {
            if let Screen::SwitchActive { error, .. } = &mut self.screen {
                *error = Some(
                    self.store_error
                        .clone()
                        .unwrap_or_else(|| "store unavailable".to_owned()),
                );
            }
            return;
        };

        let context = match self
            .runtime
            .block_on(active_context_from_session(store.as_ref(), &self.clock))
        {
            Ok(context) => context,
            Err(err) => {
                if let Screen::SwitchActive { error, .. } = &mut self.screen {
                    *error = Some(err.to_string());
                }
                return;
            }
        };

        let result = self.runtime.block_on(self.handlers.switch_active_account(
            store.as_ref(),
            &context,
            SwitchActiveAccountRequest { target_account_id },
        ));
        match result {
            Ok(response) => {
                if let Err(err) = set_active_account(response.active_account_id) {
                    tracing::error!(target: "tanren_tui", error = %err, "set active account");
                    let projected = AccountErrorProjection::internal();
                    if let Screen::SwitchActive { error, .. } = &mut self.screen {
                        *error = Some(format!("{}: {}", projected.code, projected.summary));
                    }
                    return;
                }
                self.screen = Screen::Outcome(switch_active_outcome(&response));
            }
            Err(reason) => {
                if let Screen::SwitchActive { error, .. } = &mut self.screen {
                    *error = Some(render_error(&reason));
                }
            }
        }
    }

    fn submit(&mut self, kind: FormKind) {
        match kind {
            FormKind::SignUp => self.submit_sign_up(),
            FormKind::SignIn => self.submit_sign_in(),
            FormKind::AcceptInvitation => self.submit_accept_invitation(),
            FormKind::SwitchActive => self.submit_switch_active(),
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
            Screen::SwitchActive {
                accounts,
                selected,
                error,
            } => draw::draw_switch_active(frame, area, accounts, *selected, error.as_deref()),
            Screen::Outcome(view) => draw::draw_outcome(frame, area, view),
        }
    }
}
