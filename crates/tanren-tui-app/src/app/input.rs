use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::{
    accept_invitation_fields, check_organization_permission_fields, create_organization_fields,
    list_organization_members_fields, list_organizations_fields, sign_in_fields, sign_up_fields,
};
use crate::{FormState, MenuChoice};

use super::{FormAction, Screen};

pub(super) fn handle_menu_key(
    selected: &mut usize,
    key: KeyEvent,
    next: &mut Option<Screen>,
) -> bool {
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
                MenuChoice::CreateOrganization => {
                    Screen::CreateOrganization(FormState::new(create_organization_fields()))
                }
                MenuChoice::ListOrganizations => {
                    Screen::ListOrganizations(FormState::new(list_organizations_fields()))
                }
                MenuChoice::CheckOrganizationPermission => Screen::CheckOrganizationPermission(
                    FormState::new(check_organization_permission_fields()),
                ),
                MenuChoice::ListOrganizationMembers => Screen::ListOrganizationMembers(
                    FormState::new(list_organization_members_fields()),
                ),
            });
        }
        _ => {}
    }
    false
}

pub(super) fn handle_form_key(state: &mut FormState, key: KeyEvent) -> Option<FormAction> {
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

pub(super) fn is_press(key: &KeyEvent) -> bool {
    use crossterm::event::KeyEventKind;
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}
