//! `SeaORM` entity definitions. Crate-private by architectural rule: row shapes
//! must not leak across the workspace dependency boundary. `check-deps`
//! mechanically rejects `pub mod events` here.

pub(crate) mod account_sessions;
pub(crate) mod accounts;
pub(crate) mod events;
pub(crate) mod invitations;
pub(crate) mod memberships;
pub(crate) mod user_config_values;
pub(crate) mod user_credentials;
