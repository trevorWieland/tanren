//! Input-event dispatch helpers for the TUI screen state machine.
//!
//! Split out of `app.rs` so that file stays under the workspace 500-line
//! budget. Houses the key-interpretation enums (`FormKind`, `FormAction`,
//! `Effect`), menu navigation, and form key handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::Screen;
use crate::ui::{
    accept_invitation_fields, create_organization_fields, list_organizations_fields,
    org_admin_probe_fields, sign_in_fields, sign_up_fields,
};
use crate::{FormState, MenuChoice};

#[derive(Debug, Clone, Copy)]
pub(crate) enum FormKind {
    SignUp,
    SignIn,
    AcceptInvitation,
    CreateOrganization,
    ListOrganizations,
    OrgAdminProbe,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum FormAction {
    Submit,
    Cancel,
}

#[derive(Debug)]
pub(crate) enum Effect {
    None,
    Exit,
    ReplaceScreen(Screen),
    Form(FormAction, FormKind),
}

pub(crate) fn form_effect(state: &mut FormState, key: KeyEvent, kind: FormKind) -> Effect {
    handle_form_key(state, key).map_or(Effect::None, |a| Effect::Form(a, kind))
}

pub(crate) fn handle_menu_key(
    selected: &mut usize,
    key: KeyEvent,
    next: &mut Option<Screen>,
) -> bool {
    match key.code {
        KeyCode::Char('q' | 'Q') | KeyCode::Esc => return true,
        KeyCode::Up => {
            *selected = if *selected == 0 {
                MenuChoice::ALL.len() - 1
            } else {
                *selected - 1
            };
        }
        KeyCode::Down | KeyCode::Tab => {
            *selected = (*selected + 1) % MenuChoice::ALL.len();
        }
        KeyCode::Enter => {
            *next = Some(match MenuChoice::ALL[*selected] {
                MenuChoice::SignUp => Screen::SignUp(FormState::new(sign_up_fields())),
                MenuChoice::SignIn => Screen::SignIn(FormState::new(sign_in_fields())),
                MenuChoice::AcceptInvitation => {
                    Screen::AcceptInvitation(FormState::new(accept_invitation_fields()))
                }
                MenuChoice::CreateOrganization => {
                    Screen::CreateOrganization(FormState::new(create_organization_fields()))
                }
                MenuChoice::ListOrganizations => {
                    Screen::ListOrganizations(FormState::new(list_organizations_fields()))
                }
                MenuChoice::OrgAdminProbe => {
                    Screen::OrgAdminProbe(FormState::new(org_admin_probe_fields()))
                }
            });
        }
        _ => {}
    }
    false
}

pub(crate) fn handle_form_key(state: &mut FormState, key: KeyEvent) -> Option<FormAction> {
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

pub(crate) fn is_press(key: &KeyEvent) -> bool {
    use crossterm::event::KeyEventKind;
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

pub(crate) fn is_ctrl_c(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c'))
}
