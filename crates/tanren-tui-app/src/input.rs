use crossterm::event::{KeyCode, KeyEvent};

use crate::app::Screen;
use crate::ui::{
    accept_invitation_fields, active_project_fields, connect_repository_fields,
    create_project_fields, list_projects_fields, sign_in_fields, sign_up_fields,
};
use crate::{FormState, MenuChoice};

#[derive(Debug, Clone, Copy)]
pub(crate) enum FormKind {
    SignUp,
    SignIn,
    AcceptInvitation,
    ConnectRepository,
    CreateProject,
    ListProjects,
    ActiveProject,
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
                MenuChoice::ConnectRepository => {
                    Screen::ConnectRepository(FormState::new(connect_repository_fields()))
                }
                MenuChoice::CreateProject => {
                    Screen::CreateProject(FormState::new(create_project_fields()))
                }
                MenuChoice::ListProjects => {
                    Screen::ListProjects(FormState::new(list_projects_fields()))
                }
                MenuChoice::ActiveProject => {
                    Screen::ActiveProject(FormState::new(active_project_fields()))
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
        KeyCode::Up => {
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
