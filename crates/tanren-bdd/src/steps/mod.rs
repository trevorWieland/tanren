//! Step-definition modules. `account` proves B-0043,
//! `user_configuration` proves B-0048, and `user_credentials` proves
//! B-0125. Additional R-* slices add their modules here and cucumber
//! registers their macros globally.

pub mod account;
pub mod user_configuration;
pub mod user_credentials;
