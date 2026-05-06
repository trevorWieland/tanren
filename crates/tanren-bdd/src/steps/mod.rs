//! Step-definition modules. R-0001 ships `account` (the account flow
//! that proves B-0043). R-0019 adds `project` (connect/create/active
//! for B-0025 and B-0026). Future R-* slices add their own modules
//! here and the macros register globally.

pub mod account;
pub mod project;
