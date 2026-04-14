//! Map between `SeaORM` entity `Model` rows and `tanren-domain` types.
//!
//! Infallible directions (domain → `ActiveModel`) use [`From`]. Fallible
//! directions (`Model` → domain types, where malformed JSON is possible
//! in principle) use [`TryFrom`] returning [`crate::StoreError`]. This
//! keeps the trait implementations free of conversion boilerplate and
//! funnels every failure through [`StoreError::Conversion`].
//!
//! Every converter uses the `serde_json::Value` path (never string
//! intermediate) — Lane 0.2's `event_value_roundtrip` test certifies
//! that every domain variant survives this path losslessly, and it's
//! the exact API `SeaORM` uses for `JsonBinary` columns.
//!
//! [`StoreError::Conversion`]: crate::StoreError::Conversion

pub(crate) mod dispatch;
pub(crate) mod events;
pub(crate) mod step;
pub(crate) mod validate;
