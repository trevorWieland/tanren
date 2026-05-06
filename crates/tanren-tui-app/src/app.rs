//! TUI screen state machine and submit dispatch.

use std::env;
use std::io::Stdout;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tanren_app_services::{Handlers, Store};
use tanren_identity_policy::AccountId;
use tokio::runtime::Runtime;

use crate::draw;
use crate::ui::{
    Effect, FormAction, FormKind, accept_invitation_outcome, detail_item_count, handle_form_key,
    handle_menu_key, is_press, parse_accept_invitation, parse_sign_in, parse_sign_up, render_error,
    sign_in_outcome, sign_up_outcome, toggle_detail_spec,
};
use crate::{FormState, ProjectDetailState, ProjectListState};

const DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Debug)]
pub(crate) enum Screen {
    Menu { selected: usize },
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
    Outcome(OutcomeView),
    ProjectList(ProjectListState),
    ProjectDetail(Box<ProjectDetailState>),
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
                Ok(s) => (Some(Arc::new(s)), None),
                Err(e) => (None, Some(format!("store unavailable: {e}"))),
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
            if !event::poll(Duration::from_millis(200)).context("poll events")? {
                continue;
            }
            let Event::Key(key) = event::read().context("read event")? else {
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
                    if matches!(screen, Screen::ProjectList(_)) && self.account_id.is_some() {
                        self.load_projects();
                        Effect::None
                    } else if matches!(screen, Screen::ProjectList(_)) {
                        Effect::None
                    } else {
                        Effect::ReplaceScreen(Box::new(screen))
                    }
                } else {
                    Effect::None
                }
            }
            Screen::Outcome(_) => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | 'Q')
                ) {
                    if self.account_id.is_some() {
                        self.load_projects();
                    } else {
                        self.screen = Screen::Menu { selected: 0 };
                    }
                }
                Effect::None
            }
            Screen::ProjectList(state) => {
                match key.code {
                    KeyCode::Char('q' | 'Q') | KeyCode::Esc => {
                        self.screen = Screen::Menu { selected: 0 };
                    }
                    KeyCode::Up => state.selected = state.selected.saturating_sub(1),
                    KeyCode::Down if !state.projects.is_empty() => {
                        state.selected = (state.selected + 1).min(state.projects.len() - 1);
                    }
                    KeyCode::Enter => self.switch_project(),
                    _ => {}
                }
                Effect::None
            }
            Screen::ProjectDetail(state) => {
                match key.code {
                    KeyCode::Char('q' | 'Q') | KeyCode::Esc => self.load_projects(),
                    KeyCode::Up => state.selected = state.selected.saturating_sub(1),
                    KeyCode::Down => {
                        state.selected =
                            (state.selected + 1).min(detail_item_count(state).saturating_sub(1));
                    }
                    KeyCode::Enter => toggle_detail_spec(state),
                    _ => {}
                }
                Effect::None
            }
            Screen::SignUp(state) => match handle_form_key(state, key) {
                Some(a) => Effect::Form(a, FormKind::SignUp),
                None => Effect::None,
            },
            Screen::SignIn(state) => match handle_form_key(state, key) {
                Some(a) => Effect::Form(a, FormKind::SignIn),
                None => Effect::None,
            },
            Screen::AcceptInvitation(state) => match handle_form_key(state, key) {
                Some(a) => Effect::Form(a, FormKind::AcceptInvitation),
                None => Effect::None,
            },
        };
        match effect {
            Effect::None => false,
            Effect::Exit => true,
            Effect::ReplaceScreen(screen) => {
                self.screen = *screen;
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
            let msg = self
                .store_error
                .clone()
                .unwrap_or_else(|| "store unavailable".to_owned());
            if let Some(st) = self.active_form_mut() {
                st.error = Some(msg);
            }
            return;
        };
        let h = &self.handlers;
        match kind {
            FormKind::SignUp => {
                let p = {
                    let Screen::SignUp(st) = &self.screen else {
                        return;
                    };
                    parse_sign_up(st)
                };
                let req = match p {
                    Ok(r) => r,
                    Err(m) => {
                        if let Screen::SignUp(st) = &mut self.screen {
                            st.error = Some(m);
                        }
                        return;
                    }
                };
                match self.runtime.block_on(h.sign_up(store.as_ref(), req)) {
                    Ok(resp) => {
                        self.account_id = Some(resp.account.id);
                        self.screen = Screen::Outcome(sign_up_outcome(&resp));
                    }
                    Err(reason) => {
                        if let Screen::SignUp(st) = &mut self.screen {
                            st.error = Some(render_error(reason));
                        }
                    }
                }
            }
            FormKind::SignIn => {
                let p = {
                    let Screen::SignIn(st) = &self.screen else {
                        return;
                    };
                    parse_sign_in(st)
                };
                let req = match p {
                    Ok(r) => r,
                    Err(m) => {
                        if let Screen::SignIn(st) = &mut self.screen {
                            st.error = Some(m);
                        }
                        return;
                    }
                };
                match self.runtime.block_on(h.sign_in(store.as_ref(), req)) {
                    Ok(resp) => {
                        self.account_id = Some(resp.account.id);
                        self.screen = Screen::Outcome(sign_in_outcome(&resp));
                    }
                    Err(reason) => {
                        if let Screen::SignIn(st) = &mut self.screen {
                            st.error = Some(render_error(reason));
                        }
                    }
                }
            }
            FormKind::AcceptInvitation => {
                let p = {
                    let Screen::AcceptInvitation(st) = &self.screen else {
                        return;
                    };
                    parse_accept_invitation(st)
                };
                let req = match p {
                    Ok(r) => r,
                    Err(m) => {
                        if let Screen::AcceptInvitation(st) = &mut self.screen {
                            st.error = Some(m);
                        }
                        return;
                    }
                };
                match self
                    .runtime
                    .block_on(h.accept_invitation(store.as_ref(), req))
                {
                    Ok(resp) => {
                        self.account_id = Some(resp.account.id);
                        self.screen = Screen::Outcome(accept_invitation_outcome(&resp));
                    }
                    Err(reason) => {
                        if let Screen::AcceptInvitation(st) = &mut self.screen {
                            st.error = Some(render_error(reason));
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
            Screen::ProjectList(state) => draw::draw_project_list(frame, area, state),
            Screen::ProjectDetail(state) => draw::draw_project_detail(frame, area, state),
        }
    }

    fn load_projects(&mut self) {
        let (projects, error) = match (self.account_id, self.store.as_ref()) {
            (None, _) => (vec![], Some("Not signed in.".to_owned())),
            (Some(_), None) => (
                vec![],
                Some(
                    self.store_error
                        .clone()
                        .unwrap_or_else(|| "store unavailable".to_owned()),
                ),
            ),
            (Some(aid), Some(store)) => match self
                .runtime
                .block_on(self.handlers.list_projects(store.as_ref(), aid))
            {
                Ok(p) => (p, None),
                Err(e) => (vec![], Some(render_error(e))),
            },
        };
        self.screen = Screen::ProjectList(ProjectListState {
            projects,
            selected: 0,
            error,
        });
    }

    fn switch_project(&mut self) {
        let Some(account_id) = self.account_id else {
            return;
        };
        let (project, project_id) = {
            let Screen::ProjectList(ref state) = self.screen else {
                return;
            };
            let Some(project) = state.projects.get(state.selected).cloned() else {
                return;
            };
            let pid = project.id;
            (project, pid)
        };
        let Some(store) = self.store.clone() else {
            self.screen = Screen::ProjectDetail(Box::new(ProjectDetailState {
                project,
                scoped: None,
                selected: 0,
                detail_spec: None,
                error: self.store_error.clone(),
            }));
            return;
        };
        match self.runtime.block_on(self.handlers.switch_active_project(
            store.as_ref(),
            account_id,
            project_id,
        )) {
            Ok(response) => {
                self.screen = Screen::ProjectDetail(Box::new(ProjectDetailState {
                    project: response.project,
                    scoped: Some(response.scoped),
                    selected: 0,
                    detail_spec: None,
                    error: None,
                }));
            }
            Err(err) => {
                self.screen = Screen::ProjectDetail(Box::new(ProjectDetailState {
                    project,
                    scoped: None,
                    selected: 0,
                    detail_spec: None,
                    error: Some(render_error(err)),
                }));
            }
        }
    }
}
