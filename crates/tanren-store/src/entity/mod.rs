//! `SeaORM` entity definitions. Crate-private by architectural rule: row shapes
//! must not leak across the workspace dependency boundary. `check-deps`
//! mechanically rejects `pub mod events` here.

pub(crate) mod account_sessions;
pub(crate) mod accounts;
pub(crate) mod events;
pub(crate) mod invitations;
pub(crate) mod memberships;
pub(crate) mod organization_create_idempotency;
pub(crate) mod organization_permission_grants;
pub(crate) mod organization_secret_values;
pub(crate) mod organization_secrets;
pub(crate) mod organizations;
