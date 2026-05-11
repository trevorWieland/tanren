//! Body-size limit for test-hook routes.
//!
//! Axum's [`DefaultBodyLimit`] layer rejects oversized requests at the
//! framework level — *before* any `Json` extractor allocates and
//! parses bytes. The constant here is deliberately generous (fixture
//! seeding payloads are small) while still bounding worst-case
//! allocation.

/// Maximum inbound body size accepted by every test-hook route.
///
/// Chosen to be comfortably above any legitimate fixture payload while
/// still low enough to block accidental or adversarial giant bodies
/// at the framework level. When the limit is exceeded Axum returns
/// `413 Payload Too Large` without invoking the handler.
pub(crate) const MAX_JSON_BODY_BYTES: usize = 65_536; // 64 KiB
