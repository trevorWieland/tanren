//! Tanren terminal UI.
//!
//! R-0001 (S-08) replaces F-0001's placeholder loop with a minimal screen
//! router for the account-flow surface: a top-level menu offers `sign up`,
//! `sign in`, and `accept invitation`. Each form screen collects the
//! identifier + password (and, for invitation, the token) and submits via
//! [`Handlers`] — the same seam every other interface routes through. The
//! resulting `SignUpResponse` / `SignInResponse` / `AcceptInvitationResponse`
//! is rendered on an outcome screen; an `AccountFailureReason` is rendered
//! as a one-line, user-readable error on the active form.

mod draw;
mod ui;

use ui::{
    accept_invitation_fields, accept_invitation_outcome, parse_accept_invitation, parse_sign_in,
    parse_sign_up, render_error, sign_in_fields, sign_in_outcome, sign_up_fields, sign_up_outcome,
};

use std::env;
use std::io::{Stdout, stdout};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tanren_app_services::{Handlers, Store};
use tokio::runtime::Runtime;

const DATABASE_URL_ENV: &str = "DATABASE_URL";

fn main() -> Result<()> {
    let mut terminal = setup_terminal().context("setup terminal")?;
    let app_result = App::new().and_then(|mut app| app.run(&mut terminal));
    let teardown_result = teardown_terminal(&mut terminal).context("teardown terminal");
    // Run errors are the more interesting signal for the user; surface them
    // first. If only teardown failed, surface that.
    app_result.and(teardown_result)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    // Each step that mutates terminal state must roll back on failure of any
    // later step in setup; otherwise the user is left in raw / alternate
    // mode with no way to recover. Build the terminal incrementally and
    // restore on the first failure.
    enable_raw_mode().context("enable raw mode")?;
    let mut out = stdout();
    if let Err(err) = execute!(out, EnterAlternateScreen).context("enter alternate screen") {
        let _ = disable_raw_mode();
        return Err(err);
    }
    let backend = CrosstermBackend::new(out);
    match Terminal::new(backend).context("construct terminal") {
        Ok(terminal) => Ok(terminal),
        Err(err) => {
            let _ = execute!(stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            Err(err)
        }
    }
}

fn teardown_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).context("leave alternate screen")?;
    terminal.show_cursor().context("show cursor")?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuChoice {
    SignUp,
    SignIn,
    AcceptInvitation,
}

impl MenuChoice {
    pub(crate) const ALL: [Self; 3] = [Self::SignUp, Self::SignIn, Self::AcceptInvitation];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SignUp => "Sign up",
            Self::SignIn => "Sign in",
            Self::AcceptInvitation => "Accept invitation",
        }
    }
}

#[derive(Debug)]
pub(crate) struct FormState {
    pub(crate) fields: Vec<FormField>,
    pub(crate) focus: usize,
    pub(crate) error: Option<String>,
}

#[derive(Debug)]
pub(crate) struct FormField {
    pub(crate) label: &'static str,
    pub(crate) secret: bool,
    pub(crate) value: String,
}

impl FormState {
    fn new(fields: Vec<FormField>) -> Self {
        Self {
            fields,
            focus: 0,
            error: None,
        }
    }

    fn cycle_focus(&mut self, forward: bool) {
        if self.fields.is_empty() {
            return;
        }
        let len = self.fields.len();
        self.focus = if forward {
            (self.focus + 1) % len
        } else {
            (self.focus + len - 1) % len
        };
    }

    fn push_char(&mut self, c: char) {
        if let Some(field) = self.fields.get_mut(self.focus) {
            field.value.push(c);
        }
    }

    fn pop_char(&mut self) {
        if let Some(field) = self.fields.get_mut(self.focus) {
            field.value.pop();
        }
    }

    pub(crate) fn value(&self, idx: usize) -> &str {
        self.fields.get(idx).map_or("", |f| f.value.as_str())
    }
}

#[derive(Debug)]
enum Screen {
    Menu { selected: usize },
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
struct App {
    runtime: Runtime,
    handlers: Handlers,
    store: Option<Arc<Store>>,
    store_error: Option<String>,
    screen: Screen,
}

impl App {
    fn new() -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("build tokio runtime")?;
        // Connect lazily so the TUI launches even when DATABASE_URL is unset
        // or the database is unreachable; the user only sees the failure on
        // submit, where the screen can render a one-line message.
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

    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
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
            // Some terminals emit `KeyEventKind::Release` events; ignore
            // anything that isn't a press so each keystroke registers once.
            if !is_press(&key) {
                continue;
            }
            if self.handle_key(key) {
                return Ok(());
            }
        }
    }

    /// Returns true when the app should exit.
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Ctrl-C is always an unconditional exit, regardless of screen.
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
        // Surface the startup-time store error on the active screen rather
        // than panicking or silently failing.
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
                    Ok(response) => self.screen = Screen::Outcome(sign_up_outcome(&response)),
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
                    Ok(response) => self.screen = Screen::Outcome(sign_in_outcome(&response)),
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
            Screen::Outcome(view) => draw::draw_outcome(frame, area, view),
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
}

/// Returns `true` when the menu should exit the app. Otherwise, may write
/// into `next` if the user picked a sub-screen to enter.
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
