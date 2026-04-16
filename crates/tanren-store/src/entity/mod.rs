//! SeaORM entity models for every table owned by the store.
//!
//! Each submodule defines one table. Entity models are intentionally
//! "dumb" rows — all mapping between these and domain types lives in
//! the `converters` module. Keeping the two separate means domain
//! evolution never requires touching SeaORM generated glue, and
//! SeaORM upgrades never require touching domain types.

pub(crate) mod actor_token_replay;
pub(crate) mod dispatch_projection;
pub(crate) mod enums;
pub(crate) mod events;
pub(crate) mod step_projection;
