//! Embedded Tanren distribution assets.
//!
//! Commands and standards profiles are compiled into the runtime so an
//! installed `tanren-cli` can bootstrap unrelated repositories without
//! depending on a checkout of this source tree.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedAsset {
    pub path: &'static str,
    pub contents: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));
