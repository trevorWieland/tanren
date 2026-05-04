//! TUI screen state machine and submit dispatch.
//!
//! Split out of `lib.rs` so the tui-app crate stays under the workspace
//! 500-line line-budget. Keeps the screen enum, the `App` struct, and
//! the form/menu key handlers together; rendering still lives in
//! `draw.rs`, form factories + outcome adapters in `ui.rs`.

use std::{env, io::Stdout, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tanren_app_services::{Handlers, Store, posture};
use tanren_contract::SetPostureRequest;
use tokio::runtime::Runtime;

use crate::ui::{
    accept_invitation_fields, accept_invitation_outcome, parse_accept_invitation, parse_sign_in,
    parse_sign_up, render_error, render_posture_error, sign_in_fields, sign_in_outcome,
    sign_up_fields, sign_up_outcome,
};
use crate::{FormState, MenuChoice, PostureScreenState, draw};

const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Debug)]
pub(crate) enum Screen {
    Menu { selected: usize },
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
    Posture(PostureScreenState),
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
    current_actor: Option<posture::Actor>,
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
            current_actor: None,
            screen: Screen::Menu { selected: 0 },
        })
    }

    pub(crate) fn set_session(&mut self, actor: posture::Actor) {
        self.current_actor = Some(actor);
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
            if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
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
                } else if matches!(key.code, KeyCode::Enter)
                    && MenuChoice::ALL[*selected] == MenuChoice::Posture
                {
                    Effect::EnterPosture
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
            Screen::Posture(state) => handle_posture_key(state, key),
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
            Effect::EnterPosture => {
                self.enter_posture_screen();
                false
            }
            Effect::SetPosture => {
                self.dispatch_set_posture();
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
                        self.screen = Screen::Outcome(accept_invitation_outcome(&response));
                    }
                    Err(reason) => {
                        if let Screen::AcceptInvitation(state) = &mut self.screen {
                            state.error = Some(render_error(reason));
                        }
                    }
                }
            }
        }
    }

    fn active_form_mut(&mut self) -> Option<&mut FormState> {
        match &mut self.screen {
            Screen::SignUp(s) | Screen::SignIn(s) | Screen::AcceptInvitation(s) => Some(s),
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
            Screen::Posture(state) => draw::draw_posture_screen(frame, area, state),
            Screen::Outcome(view) => draw::draw_outcome(frame, area, view),
        }
    }

    fn enter_posture_screen(&mut self) {
        let list = self.handlers.list_postures();
        let current = self.store.as_ref().and_then(|store| {
            self.runtime
                .block_on(self.handlers.get_posture(store.as_ref()))
                .ok()
                .map(|r| r.current.posture)
        });
        self.screen = Screen::Posture(PostureScreenState {
            postures: list.postures,
            current,
            selected: 0,
            error: None,
            attribution: None,
        });
    }

    fn dispatch_set_posture(&mut self) {
        let (target_posture, is_current) = {
            let Screen::Posture(state) = &self.screen else {
                return;
            };
            let target = state.postures[state.selected].posture;
            (target, state.current == Some(target))
        };
        if is_current {
            return;
        }
        let Some(store) = self.store.clone() else {
            let message = self
                .store_error
                .clone()
                .unwrap_or_else(|| "store unavailable".to_owned());
            if let Screen::Posture(state) = &mut self.screen {
                state.error = Some(message);
            }
            return;
        };
        let Some(actor) = self.current_actor.clone() else {
            if let Screen::Posture(state) = &mut self.screen {
                state.error =
                    Some("authentication required: sign in before changing posture".to_owned());
            }
            return;
        };
        let request = SetPostureRequest {
            posture: target_posture,
        };
        let result =
            self.runtime
                .block_on(self.handlers.set_posture(store.as_ref(), actor, request));
        let Screen::Posture(state) = &mut self.screen else {
            return;
        };
        match result {
            Ok(response) => {
                state.current = Some(response.current.posture);
                state.attribution = Some(format!(
                    "Changed by {} at {} ({} → {})",
                    response.change.actor,
                    response.change.at,
                    response.change.from,
                    response.change.to,
                ));
                state.error = None;
            }
            Err(err) => {
                state.error = Some(render_posture_error(err));
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FormKind {
    SignUp,
    SignIn,
    AcceptInvitation,
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
    EnterPosture,
    SetPosture,
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
            match choice {
                MenuChoice::SignUp => {
                    *next = Some(Screen::SignUp(FormState::new(sign_up_fields())));
                }
                MenuChoice::SignIn => {
                    *next = Some(Screen::SignIn(FormState::new(sign_in_fields())));
                }
                MenuChoice::AcceptInvitation => {
                    *next = Some(Screen::AcceptInvitation(FormState::new(
                        accept_invitation_fields(),
                    )));
                }
                MenuChoice::Posture => {}
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

fn handle_posture_key(state: &mut PostureScreenState, key: KeyEvent) -> Effect {
    match key.code {
        KeyCode::Char('q' | 'Q') | KeyCode::Esc => {
            Effect::ReplaceScreen(Screen::Menu { selected: 0 })
        }
        KeyCode::Up => {
            if state.postures.is_empty() {
                return Effect::None;
            }
            if state.selected == 0 {
                state.selected = state.postures.len() - 1;
            } else {
                state.selected -= 1;
            }
            Effect::None
        }
        KeyCode::Down => {
            if state.postures.is_empty() {
                return Effect::None;
            }
            state.selected = (state.selected + 1) % state.postures.len();
            Effect::None
        }
        KeyCode::Enter => Effect::SetPosture,
        _ => Effect::None,
    }
}
