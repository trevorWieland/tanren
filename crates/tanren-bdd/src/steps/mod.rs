//! Step-definition modules. `account` proves B-0043 and `install`
//! provides the CLI bootstrap fixture/assertion surface used by
//! B-0068/B-0070.

pub mod account;
pub mod install;
mod install_error;
pub mod install_selection;
mod install_snapshot;
mod install_steps;
mod install_upgrade;
