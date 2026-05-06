//! `SeaORM` entity definitions. Crate-private by architectural rule: row shapes
//! must not leak across the workspace dependency boundary. `check-deps`
//! mechanically rejects `pub mod events` here.

pub(crate) mod account_sessions;
pub(crate) mod accounts;
pub(crate) mod events;
pub(crate) mod invitations;
pub(crate) mod memberships;
pub(crate) mod project_dependencies;
pub(crate) mod project_loop_fixtures;
pub(crate) mod project_specs;
pub(crate) mod projects;
