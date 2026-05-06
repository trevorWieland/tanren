use std::env;
use std::io::Stdout;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tanren_app_services::{Handlers, Store};
use tanren_identity_policy::SessionToken;
use tokio::runtime::Runtime;

use crate::draw;
use crate::ui::{accept_invitation_fields, sign_in_fields, sign_up_fields};
use crate::{FormState, MenuChoice, SubmenuKind};

mod submit;

const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Debug)]
pub(crate) enum Screen {
    Menu { selected: usize },
    Submenu { kind: SubmenuKind, selected: usize },
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
    UserConfigSet(FormState),
    UserConfigRemove(FormState),
    CredentialAdd(FormState),
    CredentialUpdate(FormState),
    CredentialRemove(FormState),
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
    session_token: Option<SessionToken>,
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
            session_token: None,
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
            Screen::Submenu { kind, selected } => {
                let mut next: Option<Screen> = None;
                let mut direct: Option<DirectAction> = None;
                let exit = handle_submenu_key(*kind, selected, key, &mut next, &mut direct);
                if exit {
                    Effect::Exit
                } else if let Some(screen) = next {
                    Effect::ReplaceScreen(screen)
                } else if let Some(action) = direct {
                    Effect::DirectAction(action)
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
            Screen::UserConfigSet(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::UserConfigSet),
                None => Effect::None,
            },
            Screen::UserConfigRemove(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::UserConfigRemove),
                None => Effect::None,
            },
            Screen::CredentialAdd(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::CredentialAdd),
                None => Effect::None,
            },
            Screen::CredentialUpdate(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::CredentialUpdate),
                None => Effect::None,
            },
            Screen::CredentialRemove(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::CredentialRemove),
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
            Effect::DirectAction(action) => {
                self.execute_direct_action(action);
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
        submit::dispatch(
            &self.runtime,
            &self.handlers,
            &store,
            &mut self.session_token,
            &mut self.screen,
            kind,
        );
    }

    fn execute_direct_action(&mut self, action: DirectAction) {
        let Some(store) = self.store.clone() else {
            self.screen = Screen::Outcome(OutcomeView {
                title: "Error",
                lines: vec![
                    self.store_error
                        .clone()
                        .unwrap_or_else(|| "store unavailable".to_owned()),
                ],
            });
            return;
        };
        self.screen = submit::dispatch_direct(
            &self.runtime,
            &self.handlers,
            &store,
            self.session_token.as_ref(),
            action,
        );
    }

    fn active_form_mut(&mut self) -> Option<&mut FormState> {
        match &mut self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::UserConfigSet(s)
            | Screen::UserConfigRemove(s)
            | Screen::CredentialAdd(s)
            | Screen::CredentialUpdate(s)
            | Screen::CredentialRemove(s) => Some(s),
            _ => None,
        }
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let area = frame.area();
        match &self.screen {
            Screen::Menu { selected } => draw::draw_menu(frame, area, *selected),
            Screen::Submenu { kind, selected } => {
                draw::draw_submenu(frame, area, *kind, *selected);
            }
            Screen::SignUp(state) => draw::draw_form(frame, area, "Sign up", state),
            Screen::SignIn(state) => draw::draw_form(frame, area, "Sign in", state),
            Screen::AcceptInvitation(state) => {
                draw::draw_form(frame, area, "Accept invitation", state);
            }
            Screen::UserConfigSet(state) => draw::draw_form(frame, area, "Set config value", state),
            Screen::UserConfigRemove(state) => {
                draw::draw_form(frame, area, "Remove config value", state);
            }
            Screen::CredentialAdd(state) => draw::draw_form(frame, area, "Add credential", state),
            Screen::CredentialUpdate(state) => {
                draw::draw_form(frame, area, "Update credential", state);
            }
            Screen::CredentialRemove(state) => {
                draw::draw_form(frame, area, "Remove credential", state);
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
    UserConfigSet,
    UserConfigRemove,
    CredentialAdd,
    CredentialUpdate,
    CredentialRemove,
}

#[derive(Debug, Clone, Copy)]
enum FormAction {
    Submit,
    Cancel,
}

#[derive(Debug, Clone, Copy)]
enum DirectAction {
    ListUserConfig,
    ListCredentials,
}

#[derive(Debug)]
enum Effect {
    None,
    Exit,
    ReplaceScreen(Screen),
    Form(FormAction, FormKind),
    DirectAction(DirectAction),
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
                MenuChoice::UserConfig => Screen::Submenu {
                    kind: SubmenuKind::UserConfig,
                    selected: 0,
                },
                MenuChoice::Credentials => Screen::Submenu {
                    kind: SubmenuKind::Credentials,
                    selected: 0,
                },
            });
        }
        _ => {}
    }
    false
}

fn handle_submenu_key(
    kind: SubmenuKind,
    selected: &mut usize,
    key: KeyEvent,
    next: &mut Option<Screen>,
    direct: &mut Option<DirectAction>,
) -> bool {
    match key.code {
        KeyCode::Char('q' | 'Q') | KeyCode::Esc => {
            *next = Some(Screen::Menu { selected: 0 });
            return false;
        }
        KeyCode::Up => {
            if *selected == 0 {
                *selected = kind.choice_count() - 1;
            } else {
                *selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Tab => {
            *selected = (*selected + 1) % kind.choice_count();
        }
        KeyCode::Enter => {
            if let Some(screen) = submit::submenu_screen(kind, *selected) {
                *next = Some(screen);
            } else {
                match kind {
                    SubmenuKind::UserConfig => {
                        *direct = Some(DirectAction::ListUserConfig);
                    }
                    SubmenuKind::Credentials => {
                        *direct = Some(DirectAction::ListCredentials);
                    }
                }
            }
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
