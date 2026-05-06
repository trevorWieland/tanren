//! TUI screen state machine and submit dispatch.
//!
//! Split out of `lib.rs` so the tui-app crate stays under the workspace
//! 500-line line-budget. Keeps the screen enum, the `App` struct, and
//! the form/menu key handlers together; rendering still lives in
//! `draw.rs`, form factories + outcome adapters in `ui.rs`.

use std::env;
use std::io::Stdout;
use std::path::Path;
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
    accept_invitation_fields, accept_invitation_outcome, parse_accept_invitation, parse_sign_in,
    parse_sign_up, render_apply_error, render_error, render_preview_error, sign_in_fields,
    sign_in_outcome, sign_up_fields, sign_up_outcome, upgrade_apply_outcome, upgrade_root_fields,
};
use crate::{FormState, MenuChoice, UpgradePreviewData};

const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Debug)]
pub(crate) enum Screen {
    Menu { selected: usize },
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
    UpgradeRoot(FormState),
    UpgradePreview(UpgradePreviewData),
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
            Screen::UpgradeRoot(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::UpgradeRoot),
                None => Effect::None,
            },
            Screen::UpgradePreview(_) => match key.code {
                KeyCode::Enter => Effect::ConfirmUpgrade,
                KeyCode::Esc => Effect::ReplaceScreen(Screen::Menu { selected: 0 }),
                _ => Effect::None,
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
            Effect::ConfirmUpgrade => {
                self.confirm_upgrade();
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
        match kind {
            FormKind::UpgradeRoot => self.submit_upgrade_root(),
            FormKind::SignUp => self.submit_sign_up(),
            FormKind::SignIn => self.submit_sign_in(),
            FormKind::AcceptInvitation => self.submit_accept_invitation(),
        }
    }

    fn submit_upgrade_root(&mut self) {
        let parsed = {
            let Screen::UpgradeRoot(state) = &self.screen else {
                return;
            };
            state.value(0).to_owned()
        };
        if parsed.is_empty() {
            if let Screen::UpgradeRoot(state) = &mut self.screen {
                state.error = Some("Repository root path is required.".to_owned());
            }
            return;
        }
        let path = Path::new(&parsed);
        match tanren_app_services::preview_upgrade(path) {
            Ok(response) => {
                self.screen = Screen::UpgradePreview(UpgradePreviewData {
                    root: parsed,
                    preview: response,
                });
            }
            Err(reason) => {
                if let Screen::UpgradeRoot(state) = &mut self.screen {
                    state.error = Some(render_preview_error(&reason));
                }
            }
        }
    }

    fn submit_sign_up(&mut self) {
        let Some(store) = self.store.clone() else {
            self.set_store_unavailable();
            return;
        };
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

    fn submit_sign_in(&mut self) {
        let Some(store) = self.store.clone() else {
            self.set_store_unavailable();
            return;
        };
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

    fn submit_accept_invitation(&mut self) {
        let Some(store) = self.store.clone() else {
            self.set_store_unavailable();
            return;
        };
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

    fn set_store_unavailable(&mut self) {
        let message = self
            .store_error
            .clone()
            .unwrap_or_else(|| "store unavailable".to_owned());
        if let Some(state) = self.active_form_mut() {
            state.error = Some(message);
        }
    }

    fn active_form_mut(&mut self) -> Option<&mut FormState> {
        match &mut self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::UpgradeRoot(s) => Some(s),
            _ => None,
        }
    }

    fn confirm_upgrade(&mut self) {
        let root = match &self.screen {
            Screen::UpgradePreview(data) => data.root.clone(),
            _ => return,
        };
        let path = Path::new(&root);
        match tanren_app_services::apply_upgrade(path) {
            Ok(response) => {
                self.screen = Screen::Outcome(upgrade_apply_outcome(&response));
            }
            Err(reason) => {
                self.screen = Screen::UpgradeRoot(FormState {
                    fields: upgrade_root_fields(),
                    focus: 0,
                    error: Some(render_apply_error(reason)),
                });
            }
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
            Screen::UpgradeRoot(state) => {
                draw::draw_form(frame, area, "Upgrade assets", state);
            }
            Screen::UpgradePreview(data) => {
                draw::draw_upgrade_preview(frame, area, data);
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
    UpgradeRoot,
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
    ConfirmUpgrade,
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
                MenuChoice::UpgradeAssets => {
                    Screen::UpgradeRoot(FormState::new(upgrade_root_fields()))
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
