//! Regression fixture for `xtask check-bdd-wire-coverage`.
//!
//! Mirrors `tanren-bdd/src/steps/` and contains exactly one violation: a
//! step body that dispatches directly through
//! `tanren_app_services::Handlers::*` instead of routing through a
//! per-interface `*Harness` trait. `check-bdd-wire-coverage` must
//! reject this fixture; if it stops doing so the guard has been
//! weakened.

use tanren_app_services::Handlers;

#[when(expr = "alice signs in")]
async fn alice_signs_in() {
    let handlers = Handlers::new();
    handlers.sign_in().await;
}
