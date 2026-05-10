//! Step-definition modules.
//!
//! `account` carries B-0043 (account lifecycle) coverage.
//! `active_account`, `active_account_windows`, and
//! `active_account_invalid_sessions` carry B-0046
//! (active-account switching and window-isolation) coverage.

pub mod account;
pub mod active_account;
pub mod active_account_invalid_sessions;
pub mod active_account_windows;
