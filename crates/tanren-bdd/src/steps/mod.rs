//! Step-definition modules.
//!
//! `account` carries B-0043 (account lifecycle) coverage.
//! `active_account`, `active_account_windows`, and
//! `active_account_invalid_sessions` carry B-0046
//! (active-account switching and window-isolation) coverage.
//! `bounded_active_account` carries B-0041 (bounded active-account
//! read models) coverage.

pub mod account;
pub mod active_account;
pub mod active_account_invalid_sessions;
pub mod active_account_windows;
pub mod bounded_active_account;
pub(crate) mod event_assertions;
