pub(crate) mod api;
mod draw;
mod input;
mod witness;
use crate::FormState;
use crate::ui::{
    accept_invitation_outcome, auth_required_message, check_organization_permission_outcome,
    create_organization_outcome, list_organization_members_outcome, list_organizations_outcome,
    parse_accept_invitation, parse_check_organization_permission, parse_create_organization,
    parse_list_organization_members, parse_sign_in, parse_sign_up, permission_denied_message,
    sign_in_outcome, sign_up_outcome,
};
use api::{ActiveSession, ApiClient, ApiClientState, ApiError};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::Stdout;
use std::time::Duration;
use thiserror::Error;
use tokio::runtime::Runtime;
use witness::{
    accept_invitation_success, app_ready, check_permission_success, create_organization_success,
    list_organization_members_success, list_organizations_success, operation_error,
    sign_in_success, sign_up_success,
};
#[derive(Debug)]
pub(crate) enum Screen {
    Menu { selected: usize },
    SignUp(FormState),
    SignIn(FormState),
    AcceptInvitation(FormState),
    CreateOrganization(FormState),
    ListOrganizations(FormState),
    CheckOrganizationPermission(FormState),
    ListOrganizationMembers(FormState),
    Outcome(OutcomeView),
}
#[derive(Debug)]
pub(crate) struct OutcomeView {
    pub(crate) title: &'static str,
    pub(crate) lines: Vec<String>,
}
#[derive(Debug, Error)]
pub(crate) enum AppError {
    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
}
pub(crate) type AppResult<T> = Result<T, AppError>;
#[derive(Debug)]
pub(crate) struct App {
    runtime: Runtime,
    client_state: ApiClientState,
    active_session: Option<ActiveSession>,
    screen: Screen,
}
impl App {
    pub(crate) fn new() -> AppResult<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| AppError::Io {
                context: "build tokio runtime",
                source,
            })?;
        app_ready();
        Ok(Self {
            runtime,
            client_state: ApiClientState::from_env(),
            active_session: None,
            screen: Screen::Menu { selected: 0 },
        })
    }
    pub(crate) fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> AppResult<()> {
        loop {
            terminal
                .draw(|frame| self.draw(frame))
                .map_err(|source| AppError::Io {
                    context: "render frame",
                    source,
                })?;
            if !event::poll(Duration::from_millis(200)).map_err(|source| AppError::Io {
                context: "poll terminal events",
                source,
            })? {
                continue;
            }
            let Event::Key(key) = event::read().map_err(|source| AppError::Io {
                context: "read terminal event",
                source,
            })?
            else {
                continue;
            };
            if !input::is_press(&key) {
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
                let exit = input::handle_menu_key(selected, key, &mut next);
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
            Screen::SignUp(state) => match input::handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::SignUp),
                None => Effect::None,
            },
            Screen::SignIn(state) => match input::handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::SignIn),
                None => Effect::None,
            },
            Screen::AcceptInvitation(state) => match input::handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::AcceptInvitation),
                None => Effect::None,
            },
            Screen::CreateOrganization(state) => match input::handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::CreateOrganization),
                None => Effect::None,
            },
            Screen::ListOrganizations(state) => match input::handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::ListOrganizations),
                None => Effect::None,
            },
            Screen::CheckOrganizationPermission(state) => {
                match input::handle_form_key(state, key) {
                    Some(action) => Effect::Form(action, FormKind::CheckOrganizationPermission),
                    None => Effect::None,
                }
            }
            Screen::ListOrganizationMembers(state) => match input::handle_form_key(state, key) {
                Some(action) => Effect::Form(action, FormKind::ListOrganizationMembers),
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
        let Some(client) = self.client_state.as_client().cloned() else {
            let message = self
                .client_state
                .unavailable_message()
                .unwrap_or("api client unavailable")
                .to_owned();
            if let Some(state) = self.active_form_mut() {
                state.error = Some(message);
            }
            return;
        };
        match kind {
            FormKind::SignUp => self.submit_sign_up(&client),
            FormKind::SignIn => self.submit_sign_in(&client),
            FormKind::AcceptInvitation => self.submit_accept_invitation(&client),
            FormKind::CreateOrganization => self.submit_create_organization(&client),
            FormKind::ListOrganizations => self.submit_list_organizations(&client),
            FormKind::CheckOrganizationPermission => {
                self.submit_check_organization_permission(&client);
            }
            FormKind::ListOrganizationMembers => {
                self.submit_list_organization_members(&client);
            }
        }
    }
    fn submit_sign_up(&mut self, client: &ApiClient) {
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
        match self.runtime.block_on(client.sign_up(request)) {
            Ok(response) => {
                sign_up_success(&response.account, response.has_token);
                self.active_session = Some(ActiveSession {
                    has_token: response.has_token,
                });
                self.screen =
                    Screen::Outcome(sign_up_outcome(&response.account, response.has_token));
            }
            Err(err) => {
                if let Screen::SignUp(state) = &mut self.screen {
                    state.error = Some(Self::api_error_message("sign_up", &err));
                }
            }
        }
    }
    fn submit_sign_in(&mut self, client: &ApiClient) {
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
        match self.runtime.block_on(client.sign_in(request)) {
            Ok(response) => {
                sign_in_success(&response.account, response.has_token);
                self.active_session = Some(ActiveSession {
                    has_token: response.has_token,
                });
                self.screen =
                    Screen::Outcome(sign_in_outcome(&response.account, response.has_token));
            }
            Err(err) => {
                if let Screen::SignIn(state) = &mut self.screen {
                    state.error = Some(Self::api_error_message("sign_in", &err));
                }
            }
        }
    }
    fn submit_accept_invitation(&mut self, client: &ApiClient) {
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
        match self.runtime.block_on(client.accept_invitation(request)) {
            Ok(response) => {
                accept_invitation_success(&response);
                self.active_session = Some(ActiveSession {
                    has_token: response.has_token,
                });
                self.screen = Screen::Outcome(accept_invitation_outcome(
                    &response.account,
                    &response.joined_org.to_string(),
                    response.has_token,
                ));
            }
            Err(err) => {
                if let Screen::AcceptInvitation(state) = &mut self.screen {
                    state.error = Some(Self::api_error_message("accept_invitation", &err));
                }
            }
        }
    }
    fn submit_create_organization(&mut self, client: &ApiClient) {
        if !self.require_authenticated_session("create_organization") {
            return;
        }
        let parsed = {
            let Screen::CreateOrganization(state) = &self.screen else {
                return;
            };
            parse_create_organization(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(message) => {
                if let Screen::CreateOrganization(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        match self.runtime.block_on(client.create_organization(request)) {
            Ok(response) => {
                create_organization_success(&response);
                self.screen = Screen::Outcome(create_organization_outcome(&response));
            }
            Err(err) => {
                if let Screen::CreateOrganization(state) = &mut self.screen {
                    state.error = Some(Self::api_error_message("create_organization", &err));
                }
            }
        }
    }
    fn submit_list_organizations(&mut self, client: &ApiClient) {
        if !self.require_authenticated_session("list_organizations") {
            return;
        }
        match self.runtime.block_on(client.list_organizations()) {
            Ok(response) => {
                list_organizations_success(&response);
                self.screen = Screen::Outcome(list_organizations_outcome(&response));
            }
            Err(err) => {
                if let Screen::ListOrganizations(state) = &mut self.screen {
                    state.error = Some(Self::api_error_message("list_organizations", &err));
                }
            }
        }
    }
    fn submit_check_organization_permission(&mut self, client: &ApiClient) {
        if !self.require_authenticated_session("check_organization_permission") {
            return;
        }
        let Screen::CheckOrganizationPermission(state) = &self.screen else {
            return;
        };
        let request = match parse_check_organization_permission(state) {
            Ok(req) => req,
            Err(message) => {
                if let Screen::CheckOrganizationPermission(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        match self
            .runtime
            .block_on(client.check_organization_permission(request))
        {
            Ok(response) if response.allowed => {
                check_permission_success(&response);
                self.screen = Screen::Outcome(check_organization_permission_outcome(&response));
            }
            Ok(_) => {
                if let Screen::CheckOrganizationPermission(state) = &mut self.screen {
                    let message = permission_denied_message();
                    operation_error("check_organization_permission", &message);
                    state.error = Some(message);
                }
            }
            Err(err) => {
                if let Screen::CheckOrganizationPermission(state) = &mut self.screen {
                    state.error = Some(Self::api_error_message(
                        "check_organization_permission",
                        &err,
                    ));
                }
            }
        }
    }
    fn submit_list_organization_members(&mut self, client: &ApiClient) {
        if !self.require_authenticated_session("list_organization_members") {
            return;
        }
        let Screen::ListOrganizationMembers(state) = &self.screen else {
            return;
        };
        let request = match parse_list_organization_members(state) {
            Ok(req) => req,
            Err(message) => {
                if let Screen::ListOrganizationMembers(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        match self
            .runtime
            .block_on(client.list_organization_members(request))
        {
            Ok(response) => {
                list_organization_members_success(&response);
                self.screen = Screen::Outcome(list_organization_members_outcome(&response));
            }
            Err(err) => {
                if let Screen::ListOrganizationMembers(state) = &mut self.screen {
                    state.error = Some(Self::api_error_message("list_organization_members", &err));
                }
            }
        }
    }
    fn api_error_message(operation: &'static str, err: &ApiError) -> String {
        let message = err.message();
        operation_error(operation, &message);
        message
    }
    fn require_authenticated_session(&mut self, operation: &'static str) -> bool {
        let has_token = self
            .active_session
            .as_ref()
            .is_some_and(|session| session.has_token);
        if has_token {
            return true;
        }
        let message = auth_required_message();
        operation_error(operation, &message);
        if let Some(state) = self.active_form_mut() {
            state.error = Some(message);
        }
        false
    }
    fn active_form_mut(&mut self) -> Option<&mut FormState> {
        match &mut self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::CreateOrganization(s)
            | Screen::ListOrganizations(s)
            | Screen::CheckOrganizationPermission(s)
            | Screen::ListOrganizationMembers(s) => Some(s),
            _ => None,
        }
    }
    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        draw::draw_screen(frame, &self.screen);
    }
}
#[derive(Debug, Clone, Copy)]
enum FormKind {
    SignUp,
    SignIn,
    AcceptInvitation,
    CreateOrganization,
    ListOrganizations,
    CheckOrganizationPermission,
    ListOrganizationMembers,
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
