//! `SeaORM` entity definitions. Crate-private by architectural rule: row shapes
//! must not leak across the workspace dependency boundary. `check-deps`
//! mechanically rejects `pub mod events` here.

pub(crate) mod account_sessions;
pub(crate) mod accounts;
pub(crate) mod active_projects;
pub(crate) mod events;
pub(crate) mod invitations;
pub(crate) mod loops;
pub(crate) mod memberships;
pub(crate) mod milestones;
pub(crate) mod project_view_states;
pub(crate) mod projects;
pub(crate) mod specs;
