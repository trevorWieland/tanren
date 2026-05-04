//! Key-handling helpers extracted from `app.rs` to keep the screen state
//! machine under the workspace 500-line budget.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::app::Screen;
use crate::ui::{accept_invitation_fields, org_create_fields, sign_in_fields, sign_up_fields};
use crate::{DashboardChoice, FormState, MenuChoice};

#[derive(Debug, Clone, Copy)]
pub(crate) enum FormKind {
    SignUp,
    SignIn,
    AcceptInvitation,
    OrgCreate,
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
    NavigateToOrgList,
}

pub(crate) enum DashboardOutcome {
    None,
    Exit,
    Screen(Screen),
    NavigateToOrgList,
}

pub(crate) fn handle_menu_key(
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
            });
        }
        _ => {}
    }
    false
}

pub(crate) fn handle_dashboard_key(
    selected: &mut usize,
    key: KeyEvent,
    session_present: bool,
) -> DashboardOutcome {
    if !session_present {
        return DashboardOutcome::Screen(Screen::Menu { selected: 0 });
    }
    match key.code {
        KeyCode::Char('q' | 'Q') | KeyCode::Esc => return DashboardOutcome::Exit,
        KeyCode::Up => {
            if *selected == 0 {
                *selected = DashboardChoice::ALL.len() - 1;
            } else {
                *selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Tab => {
            *selected = (*selected + 1) % DashboardChoice::ALL.len();
        }
        KeyCode::Enter => {
            let choice = DashboardChoice::ALL[*selected];
            return match choice {
                DashboardChoice::CreateOrganization => {
                    DashboardOutcome::Screen(Screen::OrgCreate(FormState::new(org_create_fields())))
                }
                DashboardChoice::ListOrganizations => DashboardOutcome::NavigateToOrgList,
                DashboardChoice::SignOut => DashboardOutcome::Exit,
            };
        }
        _ => {}
    }
    DashboardOutcome::None
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
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}
