//! Regression fixture for `xtask check-secrets`.
//!
//! Mirrors the `tanren-contract` crate's path layout and contains
//! exactly one violation: a `password` field whose type is bare
//! `String` instead of a `secrecy` wrapper or workspace newtype.
//! `check-secrets` must reject this fixture; if it stops doing so the
//! guard has been weakened.

pub struct SignUpRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}
