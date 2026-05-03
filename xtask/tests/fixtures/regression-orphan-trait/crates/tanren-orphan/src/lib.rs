//! Regression fixture for `xtask check-orphan-traits`.
//!
//! Declares a `pub trait` and provides no `impl` for it anywhere in
//! the synthetic workspace. `check-orphan-traits` must reject this
//! fixture; if it stops doing so the guard has been weakened.

pub trait DanglingTrait {
    fn do_thing(&self);
}
