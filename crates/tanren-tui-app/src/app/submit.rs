use std::sync::Arc;

use tanren_app_services::{AuthenticatedActor, Handlers, Store};
use tanren_identity_policy::SessionToken;
use tokio::runtime::Runtime;

use crate::SubmenuKind;
use crate::config_ui::{
    create_credential_outcome, credential_add_fields, credential_remove_fields,
    credential_update_fields, list_credentials_outcome, list_user_config_outcome,
    parse_credential_add, parse_credential_remove, parse_credential_update,
    parse_user_config_remove, parse_user_config_set, remove_credential_outcome,
    remove_user_config_outcome, render_config_error, set_user_config_outcome,
    update_credential_outcome, user_config_remove_fields, user_config_set_fields,
};
use crate::ui::{
    accept_invitation_outcome, parse_accept_invitation, parse_sign_in, parse_sign_up, render_error,
    sign_in_outcome, sign_up_outcome,
};

use super::{DirectAction, FormKind, OutcomeView, Screen};

pub(super) fn dispatch(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    session_token: &mut Option<SessionToken>,
    screen: &mut Screen,
    kind: FormKind,
) {
    match kind {
        FormKind::SignUp => submit_sign_up(runtime, handlers, store, session_token, screen),
        FormKind::SignIn => submit_sign_in(runtime, handlers, store, session_token, screen),
        FormKind::AcceptInvitation => {
            submit_accept_invitation(runtime, handlers, store, session_token, screen);
        }
        FormKind::UserConfigSet => {
            submit_user_config_set(runtime, handlers, store, session_token, screen);
        }
        FormKind::UserConfigRemove => {
            submit_user_config_remove(runtime, handlers, store, session_token, screen);
        }
        FormKind::CredentialAdd => {
            submit_credential_add(runtime, handlers, store, session_token, screen);
        }
        FormKind::CredentialUpdate => {
            submit_credential_update(runtime, handlers, store, session_token, screen);
        }
        FormKind::CredentialRemove => {
            submit_credential_remove(runtime, handlers, store, session_token, screen);
        }
    }
}

fn submit_sign_up(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    session_token: &mut Option<SessionToken>,
    screen: &mut Screen,
) {
    let parsed = {
        let Screen::SignUp(state) = &*screen else {
            return;
        };
        parse_sign_up(state)
    };
    let request = match parsed {
        Ok(req) => req,
        Err(message) => {
            if let Screen::SignUp(state) = screen {
                state.error = Some(message);
            }
            return;
        }
    };
    let result = runtime.block_on(handlers.sign_up(store.as_ref(), request));
    match result {
        Ok(response) => {
            *session_token = Some(response.session.token.clone());
            *screen = Screen::Outcome(sign_up_outcome(&response));
        }
        Err(reason) => {
            if let Screen::SignUp(state) = screen {
                state.error = Some(render_error(reason));
            }
        }
    }
}

fn submit_sign_in(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    session_token: &mut Option<SessionToken>,
    screen: &mut Screen,
) {
    let parsed = {
        let Screen::SignIn(state) = &*screen else {
            return;
        };
        parse_sign_in(state)
    };
    let request = match parsed {
        Ok(req) => req,
        Err(message) => {
            if let Screen::SignIn(state) = screen {
                state.error = Some(message);
            }
            return;
        }
    };
    let result = runtime.block_on(handlers.sign_in(store.as_ref(), request));
    match result {
        Ok(response) => {
            *session_token = Some(response.session.token.clone());
            *screen = Screen::Outcome(sign_in_outcome(&response));
        }
        Err(reason) => {
            if let Screen::SignIn(state) = screen {
                state.error = Some(render_error(reason));
            }
        }
    }
}

fn submit_accept_invitation(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    session_token: &mut Option<SessionToken>,
    screen: &mut Screen,
) {
    let parsed = {
        let Screen::AcceptInvitation(state) = &*screen else {
            return;
        };
        parse_accept_invitation(state)
    };
    let request = match parsed {
        Ok(req) => req,
        Err(message) => {
            if let Screen::AcceptInvitation(state) = screen {
                state.error = Some(message);
            }
            return;
        }
    };
    let result = runtime.block_on(handlers.accept_invitation(store.as_ref(), request));
    match result {
        Ok(response) => {
            *session_token = Some(response.session.token.clone());
            *screen = Screen::Outcome(accept_invitation_outcome(&response));
        }
        Err(reason) => {
            if let Screen::AcceptInvitation(state) = screen {
                state.error = Some(render_error(reason));
            }
        }
    }
}

fn submit_user_config_set(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    session_token: &mut Option<SessionToken>,
    screen: &mut Screen,
) {
    let actor = match require_actor(runtime, handlers, store.as_ref(), session_token.as_ref()) {
        Ok(a) => a,
        Err(msg) => {
            if let Screen::UserConfigSet(s) = screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    let parsed = {
        let Screen::UserConfigSet(state) = &*screen else {
            return;
        };
        parse_user_config_set(state)
    };
    let request = match parsed {
        Ok(r) => r,
        Err(msg) => {
            if let Screen::UserConfigSet(s) = screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    match runtime.block_on(handlers.set_user_config(store.as_ref(), &actor, request)) {
        Ok(resp) => *screen = Screen::Outcome(set_user_config_outcome(&resp)),
        Err(err) => {
            if let Screen::UserConfigSet(s) = screen {
                s.error = Some(render_config_error(err));
            }
        }
    }
}

fn submit_user_config_remove(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    session_token: &mut Option<SessionToken>,
    screen: &mut Screen,
) {
    let actor = match require_actor(runtime, handlers, store.as_ref(), session_token.as_ref()) {
        Ok(a) => a,
        Err(msg) => {
            if let Screen::UserConfigRemove(s) = screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    let parsed = {
        let Screen::UserConfigRemove(state) = &*screen else {
            return;
        };
        parse_user_config_remove(state)
    };
    let request = match parsed {
        Ok(r) => r,
        Err(msg) => {
            if let Screen::UserConfigRemove(s) = screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    match runtime.block_on(handlers.remove_user_config(store.as_ref(), &actor, request)) {
        Ok(resp) => *screen = Screen::Outcome(remove_user_config_outcome(&resp)),
        Err(err) => {
            if let Screen::UserConfigRemove(s) = screen {
                s.error = Some(render_config_error(err));
            }
        }
    }
}

fn submit_credential_add(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    session_token: &mut Option<SessionToken>,
    screen: &mut Screen,
) {
    let actor = match require_actor(runtime, handlers, store.as_ref(), session_token.as_ref()) {
        Ok(a) => a,
        Err(msg) => {
            if let Screen::CredentialAdd(s) = screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    let parsed = {
        let Screen::CredentialAdd(state) = &*screen else {
            return;
        };
        parse_credential_add(state)
    };
    let request = match parsed {
        Ok(r) => r,
        Err(msg) => {
            if let Screen::CredentialAdd(s) = screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    match runtime.block_on(handlers.create_credential(store.as_ref(), &actor, request)) {
        Ok(resp) => *screen = Screen::Outcome(create_credential_outcome(&resp)),
        Err(err) => {
            if let Screen::CredentialAdd(s) = screen {
                s.error = Some(render_config_error(err));
            }
        }
    }
}

fn submit_credential_update(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    session_token: &mut Option<SessionToken>,
    screen: &mut Screen,
) {
    let actor = match require_actor(runtime, handlers, store.as_ref(), session_token.as_ref()) {
        Ok(a) => a,
        Err(msg) => {
            if let Screen::CredentialUpdate(s) = screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    let parsed = {
        let Screen::CredentialUpdate(state) = &*screen else {
            return;
        };
        parse_credential_update(state)
    };
    let request = match parsed {
        Ok(r) => r,
        Err(msg) => {
            if let Screen::CredentialUpdate(s) = screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    match runtime.block_on(handlers.update_credential(store.as_ref(), &actor, request)) {
        Ok(resp) => *screen = Screen::Outcome(update_credential_outcome(&resp)),
        Err(err) => {
            if let Screen::CredentialUpdate(s) = screen {
                s.error = Some(render_config_error(err));
            }
        }
    }
}

fn submit_credential_remove(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    session_token: &mut Option<SessionToken>,
    screen: &mut Screen,
) {
    let actor = match require_actor(runtime, handlers, store.as_ref(), session_token.as_ref()) {
        Ok(a) => a,
        Err(msg) => {
            if let Screen::CredentialRemove(s) = screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    let parsed = {
        let Screen::CredentialRemove(state) = &*screen else {
            return;
        };
        parse_credential_remove(state)
    };
    let request = match parsed {
        Ok(r) => r,
        Err(msg) => {
            if let Screen::CredentialRemove(s) = screen {
                s.error = Some(msg);
            }
            return;
        }
    };
    match runtime.block_on(handlers.remove_credential(store.as_ref(), &actor, request)) {
        Ok(resp) => *screen = Screen::Outcome(remove_credential_outcome(&resp)),
        Err(err) => {
            if let Screen::CredentialRemove(s) = screen {
                s.error = Some(render_config_error(err));
            }
        }
    }
}

pub(super) fn dispatch_direct(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Arc<Store>,
    session_token: Option<&SessionToken>,
    action: DirectAction,
) -> Screen {
    let actor = match require_actor(runtime, handlers, store.as_ref(), session_token) {
        Ok(a) => a,
        Err(msg) => {
            return Screen::Outcome(OutcomeView {
                title: "Error",
                lines: vec![msg],
            });
        }
    };
    match action {
        DirectAction::ListUserConfig => {
            match runtime.block_on(handlers.list_user_config(store.as_ref(), &actor)) {
                Ok(resp) => Screen::Outcome(list_user_config_outcome(&resp)),
                Err(err) => Screen::Outcome(OutcomeView {
                    title: "Error",
                    lines: vec![render_config_error(err)],
                }),
            }
        }
        DirectAction::ListCredentials => {
            match runtime.block_on(handlers.list_credentials(store.as_ref(), &actor)) {
                Ok(resp) => Screen::Outcome(list_credentials_outcome(&resp)),
                Err(err) => Screen::Outcome(OutcomeView {
                    title: "Error",
                    lines: vec![render_config_error(err)],
                }),
            }
        }
    }
}

fn require_actor(
    runtime: &Runtime,
    handlers: &Handlers,
    store: &Store,
    session_token: Option<&SessionToken>,
) -> Result<AuthenticatedActor, String> {
    let token =
        session_token.ok_or_else(|| "Not signed in. Sign in or sign up first.".to_owned())?;
    runtime
        .block_on(handlers.resolve_actor(store, token))
        .map_err(render_error)
}

pub(super) fn submenu_screen(kind: SubmenuKind, idx: usize) -> Option<Screen> {
    use crate::FormState;
    match kind {
        SubmenuKind::UserConfig => match idx {
            1 => Some(Screen::UserConfigSet(FormState::new(
                user_config_set_fields(),
            ))),
            2 => Some(Screen::UserConfigRemove(FormState::new(
                user_config_remove_fields(),
            ))),
            3 => Some(Screen::Menu { selected: 0 }),
            _ => None,
        },
        SubmenuKind::Credentials => match idx {
            1 => Some(Screen::CredentialAdd(FormState::new(
                credential_add_fields(),
            ))),
            2 => Some(Screen::CredentialUpdate(FormState::new(
                credential_update_fields(),
            ))),
            3 => Some(Screen::CredentialRemove(FormState::new(
                credential_remove_fields(),
            ))),
            4 => Some(Screen::Menu { selected: 0 }),
            _ => None,
        },
    }
}
