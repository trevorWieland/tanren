// Regression fixture for the tracing-init guard. The guard rejects any
// `bin/*/src/main.rs` whose body does not call into the observability
// crate's init function. This fixture's main does no such thing —
// reject it.
//
// NOTE: doc-comments on this file deliberately use `//`, NOT `//!`. The
// guard scans the AST token stream of each main.rs, and inner doc
// attributes become `#[doc = "..."]` items in the token stream. If we
// named the observability init path inside an inner-doc, the guard
// would see the substring inside the doc text and falsely conclude
// the binary initialised tracing.

fn main() {
    println!("hi");
}
