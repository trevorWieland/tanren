//! TUI screen state machine and submit dispatch.
//!
//! Split out of `lib.rs` so the tui-app crate stays under the workspace
//! 500-line line-budget. Keeps the screen enum, the `App` struct, and
//! the form/menu key handlers together; rendering still lives in
//! `draw.rs`, form factories + outcome adapters in `ui.rs`.

use std::env;
use std::io::Stdout;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tanren_app_services::project::{
    ActiveProjectQuery, ConnectExistingRepositoryCommand, CreateNewProjectCommand,
    ListVisibleProjectsQuery,
};
use tanren_app_services::{Handlers, Store};
use tanren_provider_integrations::{SourceControlProvider, production_source_control_provider};
use tokio::runtime::Runtime;

use crate::FormState;
use crate::draw;
use crate::input::{Effect, FormAction, FormKind, handle_form_key, handle_menu_key};
use crate::ui::{
    accept_invitation_outcome, active_project_outcome, connect_repository_outcome,
    create_project_outcome, list_projects_outcome, parse_accept_invitation, parse_active_project,
    parse_connect_repository, parse_create_project, parse_list_projects, parse_sign_in,
    parse_sign_up, render_error, render_project_error, sign_in_outcome, sign_up_outcome,
};

const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Debug)]
pub(crate) enum Screen {
    Menu { selected: usize },
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
    ConnectRepository(FormState),
    CreateProject(FormState),
    ListProjects(FormState),
    ActiveProject(FormState),
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
    source_control: Arc<dyn SourceControlProvider>,
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
                Err(err) => {
                    tracing::error!(error = ?err, "tui failed to connect store at startup");
                    (
                        None,
                        Some("internal_error: Tanren store is unavailable.".to_owned()),
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
            handlers: Handlers::new(),
            source_control: production_source_control_provider(),
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
            Screen::ConnectRepository(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::ConnectRepository),
                None => Effect::None,
            },
            Screen::CreateProject(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::CreateProject),
                None => Effect::None,
            },
            Screen::ListProjects(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::ListProjects),
                None => Effect::None,
            },
            Screen::ActiveProject(state) => match handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::ActiveProject),
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
            FormKind::SignUp => self.submit_sign_up(store.as_ref()),
            FormKind::SignIn => self.submit_sign_in(store.as_ref()),
            FormKind::AcceptInvitation => self.submit_accept_invitation(store.as_ref()),
            FormKind::ConnectRepository => self.submit_connect_repository(store.as_ref()),
            FormKind::CreateProject => self.submit_create_project(store.as_ref()),
            FormKind::ListProjects => self.submit_list_projects(store.as_ref()),
            FormKind::ActiveProject => self.submit_active_project(store.as_ref()),
        }
    }

    fn submit_sign_up(&mut self, store: &Store) {
        let request = match (&self.screen, FormKind::SignUp) {
            (Screen::SignUp(state), _) => match parse_sign_up(state) {
                Ok(req) => req,
                Err(message) => return self.set_form_error(FormKind::SignUp, message),
            },
            _ => return,
        };
        match self.runtime.block_on(self.handlers.sign_up(store, request)) {
            Ok(response) => self.screen = Screen::Outcome(sign_up_outcome(&response)),
            Err(reason) => self.set_form_error(FormKind::SignUp, render_error(reason)),
        }
    }

    fn submit_sign_in(&mut self, store: &Store) {
        let request = match (&self.screen, FormKind::SignIn) {
            (Screen::SignIn(state), _) => match parse_sign_in(state) {
                Ok(req) => req,
                Err(message) => return self.set_form_error(FormKind::SignIn, message),
            },
            _ => return,
        };
        match self.runtime.block_on(self.handlers.sign_in(store, request)) {
            Ok(response) => self.screen = Screen::Outcome(sign_in_outcome(&response)),
            Err(reason) => self.set_form_error(FormKind::SignIn, render_error(reason)),
        }
    }

    fn submit_accept_invitation(&mut self, store: &Store) {
        let request = match (&self.screen, FormKind::AcceptInvitation) {
            (Screen::AcceptInvitation(state), _) => match parse_accept_invitation(state) {
                Ok(req) => req,
                Err(message) => return self.set_form_error(FormKind::AcceptInvitation, message),
            },
            _ => return,
        };
        match self
            .runtime
            .block_on(self.handlers.accept_invitation(store, request))
        {
            Ok(response) => self.screen = Screen::Outcome(accept_invitation_outcome(&response)),
            Err(reason) => self.set_form_error(FormKind::AcceptInvitation, render_error(reason)),
        }
    }

    fn submit_connect_repository(&mut self, store: &Store) {
        let request = match (&self.screen, FormKind::ConnectRepository) {
            (Screen::ConnectRepository(state), _) => match parse_connect_repository(state) {
                Ok(req) => req,
                Err(message) => return self.set_form_error(FormKind::ConnectRepository, message),
            },
            _ => return,
        };
        let actor_account_id = request.owning_account_id;
        match self
            .runtime
            .block_on(self.handlers.connect_project_repository(
                store,
                self.source_control.as_ref(),
                ConnectExistingRepositoryCommand {
                    actor_account_id,
                    request,
                },
            )) {
            Ok(response) => self.screen = Screen::Outcome(connect_repository_outcome(&response)),
            Err(reason) => {
                self.set_form_error(FormKind::ConnectRepository, render_project_error(reason));
            }
        }
    }

    fn submit_create_project(&mut self, store: &Store) {
        let request = match (&self.screen, FormKind::CreateProject) {
            (Screen::CreateProject(state), _) => match parse_create_project(state) {
                Ok(req) => req,
                Err(message) => return self.set_form_error(FormKind::CreateProject, message),
            },
            _ => return,
        };
        let actor_account_id = request.owning_account_id;
        match self.runtime.block_on(self.handlers.create_project(
            store,
            self.source_control.as_ref(),
            CreateNewProjectCommand {
                actor_account_id,
                request,
            },
        )) {
            Ok(response) => self.screen = Screen::Outcome(create_project_outcome(&response)),
            Err(reason) => {
                self.set_form_error(FormKind::CreateProject, render_project_error(reason));
            }
        }
    }

    fn submit_list_projects(&mut self, store: &Store) {
        let request = match (&self.screen, FormKind::ListProjects) {
            (Screen::ListProjects(state), _) => match parse_list_projects(state) {
                Ok(req) => req,
                Err(message) => return self.set_form_error(FormKind::ListProjects, message),
            },
            _ => return,
        };
        let actor_account_id = request.owning_account_id;
        match self.runtime.block_on(self.handlers.list_visible_projects(
            store,
            ListVisibleProjectsQuery {
                actor_account_id,
                request,
            },
        )) {
            Ok(response) => self.screen = Screen::Outcome(list_projects_outcome(&response)),
            Err(reason) => {
                self.set_form_error(FormKind::ListProjects, render_project_error(reason));
            }
        }
    }

    fn submit_active_project(&mut self, store: &Store) {
        let request = match (&self.screen, FormKind::ActiveProject) {
            (Screen::ActiveProject(state), _) => match parse_active_project(state) {
                Ok(req) => req,
                Err(message) => return self.set_form_error(FormKind::ActiveProject, message),
            },
            _ => return,
        };
        let actor_account_id = request.owning_account_id;
        match self.runtime.block_on(self.handlers.active_project(
            store,
            ActiveProjectQuery {
                actor_account_id,
                request,
            },
        )) {
            Ok(response) => self.screen = Screen::Outcome(active_project_outcome(&response)),
            Err(reason) => {
                self.set_form_error(FormKind::ActiveProject, render_project_error(reason));
            }
        }
    }

    fn set_form_error(&mut self, kind: FormKind, message: String) {
        match (&mut self.screen, kind) {
            (Screen::SignUp(state), FormKind::SignUp)
            | (Screen::SignIn(state), FormKind::SignIn)
            | (Screen::AcceptInvitation(state), FormKind::AcceptInvitation)
            | (Screen::ConnectRepository(state), FormKind::ConnectRepository)
            | (Screen::CreateProject(state), FormKind::CreateProject)
            | (Screen::ListProjects(state), FormKind::ListProjects)
            | (Screen::ActiveProject(state), FormKind::ActiveProject) => {
                state.error = Some(message);
            }
            _ => {}
        }
    }

    fn active_form_mut(&mut self) -> Option<&mut FormState> {
        match &mut self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::ConnectRepository(s)
            | Screen::CreateProject(s)
            | Screen::ListProjects(s)
            | Screen::ActiveProject(s) => Some(s),
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
            Screen::ConnectRepository(state) => {
                draw::draw_form(frame, area, "Connect repository", state);
            }
            Screen::CreateProject(state) => {
                draw::draw_form(frame, area, "Create project", state);
            }
            Screen::ListProjects(state) => {
                draw::draw_form(frame, area, "List projects", state);
            }
            Screen::ActiveProject(state) => {
                draw::draw_form(frame, area, "Active project", state);
            }
            Screen::Outcome(view) => draw::draw_outcome(frame, area, view),
        }
    }
}

fn is_press(key: &KeyEvent) -> bool {
    use crossterm::event::KeyEventKind;
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}
