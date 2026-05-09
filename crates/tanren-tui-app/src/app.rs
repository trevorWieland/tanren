//! TUI screen state machine and submit dispatch.
//!
//! Split out of `lib.rs` so the tui-app crate stays under the workspace
//! 500-line line-budget. Keeps the screen enum, the `App` struct, and
//! the form/menu key handlers together; rendering still lives in
//! `draw.rs`, form factories + outcome adapters in `ui.rs`.

mod app_keys;
mod app_submit;

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

use self::app_keys::{
    handle_form_key, handle_menu_key, handle_user_config_credentials_menu_key,
    handle_user_config_menu_key, handle_user_config_settings_menu_key, is_press,
};
use crate::FormState;
use crate::draw;

const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Debug)]
pub(crate) enum Screen {
    Menu { selected: usize },
    UserConfigMenu { selected: usize },
    UserConfigSettingsMenu { selected: usize },
    UserConfigCredentialsMenu { selected: usize },
    UserConfigListSettings(FormState),
    UserConfigSetSetting(FormState),
    UserConfigRemoveSetting(FormState),
    UserConfigListCredentials(FormState),
    UserConfigAddCredential(FormState),
    UserConfigUpdateCredential(FormState),
    UserConfigRemoveCredential(FormState),
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
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
        let effect = self.effect_from_key(key);
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

    fn effect_from_key(&mut self, key: KeyEvent) -> Effect {
        match &mut self.screen {
            Screen::Menu { selected } => menu_effect(selected, key),
            Screen::UserConfigMenu { selected } => menu_effect_to(
                selected,
                key,
                handle_user_config_menu_key,
                Screen::Menu { selected: 0 },
            ),
            Screen::UserConfigSettingsMenu { selected } => menu_effect_to(
                selected,
                key,
                handle_user_config_settings_menu_key,
                Screen::UserConfigMenu { selected: 0 },
            ),
            Screen::UserConfigCredentialsMenu { selected } => menu_effect_to(
                selected,
                key,
                handle_user_config_credentials_menu_key,
                Screen::UserConfigMenu { selected: 0 },
            ),
            Screen::Outcome(_) => outcome_effect(key),
            Screen::SignUp(state) => form_effect(state, key, FormKind::SignUp),
            Screen::SignIn(state) => form_effect(state, key, FormKind::SignIn),
            Screen::AcceptInvitation(state) => form_effect(state, key, FormKind::AcceptInvitation),
            Screen::UserConfigListSettings(state) => {
                form_effect(state, key, FormKind::UserConfigListSettings)
            }
            Screen::UserConfigSetSetting(state) => {
                form_effect(state, key, FormKind::UserConfigSetSetting)
            }
            Screen::UserConfigRemoveSetting(state) => {
                form_effect(state, key, FormKind::UserConfigRemoveSetting)
            }
            Screen::UserConfigListCredentials(state) => {
                form_effect(state, key, FormKind::UserConfigListCredentials)
            }
            Screen::UserConfigAddCredential(state) => {
                form_effect(state, key, FormKind::UserConfigAddCredential)
            }
            Screen::UserConfigUpdateCredential(state) => {
                form_effect(state, key, FormKind::UserConfigUpdateCredential)
            }
            Screen::UserConfigRemoveCredential(state) => {
                form_effect(state, key, FormKind::UserConfigRemoveCredential)
            }
        }
    }

    fn dispatch_form_action(&mut self, action: FormAction, kind: FormKind) {
        match action {
            FormAction::Cancel => {
                self.screen = kind.cancel_screen();
            }
            FormAction::Submit => self.submit(kind),
        }
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let area = frame.area();
        match &self.screen {
            Screen::Menu { selected } => draw::draw_menu(frame, area, *selected),
            Screen::UserConfigMenu { selected } => draw::draw_choice_menu(
                frame,
                area,
                " user configuration ",
                "Choose a section:",
                &["Settings", "Credentials", "Back"],
                *selected,
            ),
            Screen::UserConfigSettingsMenu { selected } => draw::draw_choice_menu(
                frame,
                area,
                " user settings ",
                "Choose an action:",
                &["List", "Set", "Remove", "Back"],
                *selected,
            ),
            Screen::UserConfigCredentialsMenu { selected } => draw::draw_choice_menu(
                frame,
                area,
                " user credentials ",
                "Choose an action:",
                &["List", "Add", "Update", "Remove", "Back"],
                *selected,
            ),
            Screen::SignUp(state) => draw::draw_form(frame, area, "Sign up", state),
            Screen::SignIn(state) => draw::draw_form(frame, area, "Sign in", state),
            Screen::AcceptInvitation(state) => {
                draw::draw_form(frame, area, "Accept invitation", state);
            }
            Screen::UserConfigListSettings(state) => {
                draw::draw_form(frame, area, "List user settings", state);
            }
            Screen::UserConfigSetSetting(state) => {
                draw::draw_form(frame, area, "Set user setting", state);
            }
            Screen::UserConfigRemoveSetting(state) => {
                draw::draw_form(frame, area, "Remove user setting", state);
            }
            Screen::UserConfigListCredentials(state) => {
                draw::draw_form(frame, area, "List credentials", state);
            }
            Screen::UserConfigAddCredential(state) => {
                draw::draw_form(frame, area, "Add credential", state);
            }
            Screen::UserConfigUpdateCredential(state) => {
                draw::draw_form(frame, area, "Update credential", state);
            }
            Screen::UserConfigRemoveCredential(state) => {
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
    UserConfigListSettings,
    UserConfigSetSetting,
    UserConfigRemoveSetting,
    UserConfigListCredentials,
    UserConfigAddCredential,
    UserConfigUpdateCredential,
    UserConfigRemoveCredential,
}

impl FormKind {
    fn cancel_screen(self) -> Screen {
        match self {
            Self::SignUp | Self::SignIn | Self::AcceptInvitation => Screen::Menu { selected: 0 },
            Self::UserConfigListSettings
            | Self::UserConfigSetSetting
            | Self::UserConfigRemoveSetting => Screen::UserConfigSettingsMenu { selected: 0 },
            Self::UserConfigListCredentials
            | Self::UserConfigAddCredential
            | Self::UserConfigUpdateCredential
            | Self::UserConfigRemoveCredential => Screen::UserConfigCredentialsMenu { selected: 0 },
        }
    }
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

fn menu_effect(selected: &mut usize, key: KeyEvent) -> Effect {
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

fn menu_effect_to(
    selected: &mut usize,
    key: KeyEvent,
    handler: fn(&mut usize, KeyEvent, &mut Option<Screen>) -> bool,
    back: Screen,
) -> Effect {
    let mut next: Option<Screen> = None;
    let exit = handler(selected, key, &mut next);
    if exit {
        Effect::ReplaceScreen(back)
    } else if let Some(screen) = next {
        Effect::ReplaceScreen(screen)
    } else {
        Effect::None
    }
}

fn outcome_effect(key: KeyEvent) -> Effect {
    if matches!(
        key.code,
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | 'Q')
    ) {
        Effect::ReplaceScreen(Screen::Menu { selected: 0 })
    } else {
        Effect::None
    }
}

fn form_effect(state: &mut FormState, key: KeyEvent, kind: FormKind) -> Effect {
    match handle_form_key(state, key) {
        Some(action) => Effect::Form(action, kind),
        None => Effect::None,
    }
}
