//! Regression fixture for `xtask check-newtype-ids`.
//!
//! Mirrors `tanren-contract/src/` and contains exactly one violation:
//! a struct field typed as bare `Uuid` instead of a workspace newtype.
//! `check-newtype-ids` must reject this fixture; if it stops doing so
//! the guard has been weakened.

use uuid::Uuid;

pub struct AccountRecord {
    pub id: Uuid,
    pub display_name: String,
}
