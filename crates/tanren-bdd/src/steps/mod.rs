//! Step-definition modules. R-0001 ships `account` (the account flow
//! that proves B-0043). R-0006 adds `account_join` for B-0045
//! existing-account join-organization steps. Future R-* slices add
//! their own modules here; the macros register globally.

pub mod account;
pub mod account_join;
