//! `SeaORM` entity definitions. Crate-private by architectural rule: row shapes
//! must not leak across the workspace dependency boundary. `check-deps`
//! mechanically rejects `pub mod events` here.

pub(crate) mod account_sessions;
pub(crate) mod accounts;
pub(crate) mod deployment_postures;
pub(crate) mod events;
pub(crate) mod invitations;
pub(crate) mod memberships;
pub(crate) mod provider_connection_reachable_repos;
pub(crate) mod provider_connections;
